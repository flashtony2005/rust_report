//! `ds.select(...)` 全参数矩阵与双态求值（M5-01）。
//!
//! 算法正文：`docs/spec-m5.md` §2.2（参数矩阵）/ §2.3（双态）/ §3（探针真值）。
//! 本目录是产品 `DSSelect.calculate`（`reportlite/03-model/.../dsfunction/DSSelect.java`，
//! 全文 211 行）的逐行移植，外加三个协作者：
//! `Group.filter/distinct/sort/evaluateRows/getRows`（`dataset/Group.java`）、
//! `SortDsValue.sort`（`dataset/SortDsValue.java`）、`ParamInfo2.parse`
//! （`model/expression/ParamInfo2.java`）。
//!
//! # 拆分（G4 行数门禁，原单文件 583 行）
//!
//! | 文件 | 内容 | 产品对应 |
//! |------|------|---------|
//! | [`mod.rs`]（本文件） | 共享辅助 + `ds_select` 主入口（双态分派） | `DSSelect.calculate:146-208` |
//! | [`decode`] | 参数矩阵解码（`SelectArgs` 全字段） | `DSSelect.calculate:36-145` |
//! | [`rows`] | 定序 `evaluateRows` + `;` 多键排序 | `Group.evaluateRows` / `SortDsValue.sort` |
//!
//! # 参数矩阵（`DSSelect.java:36-145`）
//!
//! | 模板形态 | 树形 | 解码结果 |
//! |---------|------|---------|
//! | `select(x)` | Leaf | selectExp |
//! | `select(x:1)` | Colon[x, 1] | + order=1 |
//! | `select(x; s1:1, s2:-1)` | Semicolon[x, Comma[Colon[s1,1], Colon[s2,-1]]] | + sortExps/isDescs |
//! | `select(x, f, n, d)` | Comma[x, f, n, d] | + flt/num/distinct |
//! | `select(x:1, f, , d)` | Comma[Colon[x,1], f, null, d] | 混合（官方主子表样例） |
//!
//! 空段 = null `IParam`（`ParamParser.java:39-41`）⇒ 树里是 `None`。
//!
//! # 双态（`DSSelect.java:146-208`）
//!
//! - **Row 态**（current 是 Row；`option` 不含 `r`）：只对**当前行**测过滤，
//!   命中 → `setValue` + 返回该行；未命中 → `getNullRow()`（本仓
//!   [`NULL_ROW_NOTE`] 旁路）。**num/distinct/order/sortExps 全部不生效**
//!   （产品在 Row 分支里根本没读它们，探针 T1 实证）。
//! - **Root 态**：`filter → distinct → (sortExps ? sort : evaluate+orderSort)`
//!   → size/num 逻辑：`size==0 → NullRow`；`size==1 || num==1 → 首行`
//!   （`z` 选项取末行）；`num==-1 || num>=size → 全量`（`z` 倒序）；
//!   否则前 `num` 行（`z` 取末 num 行再倒序）。
//!
//! # 三处「不等价简化」警告（复刻时刻意保留的分叉）
//!
//! 1. **`;` 多排序先 sort 后 evaluate，`:` 次序先 evaluate 后 sort**
//!    （`:161-171`）—— 前者按排序表达式求值，后者按**选列值**排序。
//!    合并成一步会改掉 `:` 形态的语义。
//! 2. **distinct 的键是 distinctExp 的值、且首现者胜出**
//!    （`Group.distinct:803-826` 的 HashMap put 语义）—— 不是"排序后去重"。
//! 3. **Row 态不过滤 num/distinct/order**：产品 Row 分支（`:148-154`）只读
//!    `fltExp` 与 `selectExp`，多写的参数**静默失效**（探针 T1 的
//!    num/去重/降序变体实证）。

mod decode;
mod rows;

use crate::error::{EngineError, Result};
use crate::expr::ast::{Call, Expr};
use crate::expr::ds::DsCur;
use crate::expr::eval::{evaluate, EvalCtx, NULL_ROW_NOTE};
use crate::expr::num::as_bool;
use crate::value::Value;

use decode::{decode, SelectArgs};
use rows::{eval_rows_idx, multi_key_sort};

/// 数据集缺失的出口（`DSFunction` 基类在 `calculate` 前由引擎保证 ds 非空；
/// 本仓求值期才绑定 ⇒ 显式报，与 `dsfn::ds_column` 同策略）。
pub(super) fn missing(dsn: &str) -> EngineError {
    EngineError::MissingDataset {
        name: dsn.to_string(),
    }
}

/// `UnknownSymbol.setDS`（`UnknownSymbol.java:38-41`）的 AST 同构。
///
/// 产品在 `ParamParser._$1` 建表达式时把 `ds` 传进 `new Expression(cs, ds, ...)`
/// ⇒ 参数树里所有**裸列名**在建树期就绑定到本调用的数据集，求值期走
/// `ds.getCurrentData(colNo)`。本仓的 [`Expr::Unknown`] 不携带 ds 信息
/// （独立出现会被 `evaluate` 拒绝，`eval.rs:179`）⇒ 递归改写成
/// [`Expr::DsColumn`]：Row 态取值与 `getCurrentData` 逐字等价
/// （`dsfn::ds_column` 与 `dsfn::ds_current_data` 同一实现路径）；
/// Root 态走产品 currentRow 光标（`DataSet.java:395-410`，见
/// `dsfn::root_current_data`）。
///
/// ⚠ 产品对「绑了 ds 但名字不是列」的名字还会回落参数名查找、最后才抛
/// `unknownExpression`（`UnknownSymbol.java:29-37/48-51`）；本仓改写后直接报
/// [`EngineError::UnknownCell`]。两者都显式失败，无静默分歧。
pub(super) fn bind_ds(e: &Expr, dsn: &str) -> Expr {
    match e {
        Expr::Unknown(col) => Expr::DsColumn {
            ds: dsn.to_string(),
            col: col.clone(),
        },
        Expr::Unary { op, rhs } => Expr::Unary {
            op: *op,
            rhs: Box::new(bind_ds(rhs, dsn)),
        },
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op: *op,
            lhs: Box::new(bind_ds(lhs, dsn)),
            rhs: Box::new(bind_ds(rhs, dsn)),
        },
        other => other.clone(),
    }
}

/// 绑定数据集后求值（产品「参数树整体带 ds」的统一入口）。
pub(super) fn eval_bound(e: &Expr, dsn: &str, ctx: &mut dyn EvalCtx) -> Result<Value> {
    evaluate(&bind_ds(e, dsn), ctx)
}

/// `ds.select(...)` 主入口（`DSSelect.calculate`，`:27-209`）。
pub(crate) fn ds_select(c: &Call, dsn: &str, ctx: &mut dyn EvalCtx) -> Result<Value> {
    let sel: SelectArgs = decode(c, ctx)?;
    let opt = c.option.as_deref().unwrap_or("");
    // `option` 含 `r`(114) ⇒ 跳过 Row 分支强制走 Root（`:146-158`）
    let force_root = opt.contains('r');
    // `z`(122) ⇒ 单值取末行、List 倒序（`:178-208`）
    let rev = opt.contains('z');

    let is_row = {
        let set = ctx.ds(dsn).ok_or_else(|| missing(dsn))?;
        matches!(set.current, DsCur::Row(_)) && !force_root
    };

    // ── Row 态（`:148-154`）──
    if is_row {
        // current 是空行哨兵（上游 select 过滤未命中钉下的 `getNullRow()`，
        // calc 的 `mirror` 投影成 `DsCur::Row(NULL_ROW)`）：产品会对 NullRow
        // `test`/`setValue`（rowNo=-1 的边缘，无夹具）⇒ 本仓显式按空行身份
        // 返回，**不 panic**（`selected[i]` 直写会越界）。
        let row_i = {
            let set = ctx.ds(dsn).expect("上面刚取到");
            set.current_row_checked()
        };
        let Some(i) = row_i else {
            if let Some(di) = ctx.ds_index(dsn) {
                ctx.note_ds_row(di, NULL_ROW_NOTE);
            }
            return Ok(Value::Null);
        };
        if let Some(flt) = &sel.flt_exp {
            // `Row.test`（`Row.java:84-92`）：setCurrent(this)（当前指针已是该行）
            // + 求值必须是 Boolean，否则产品抛 `error.Row.test`。
            let v = eval_bound(flt, dsn, ctx)?;
            if !as_bool(&v)? {
                // 未命中 ⇒ `getNullRow()`：值空 + 空行身份旁路
                if let Some(di) = ctx.ds_index(dsn) {
                    ctx.note_ds_row(di, NULL_ROW_NOTE);
                }
                return Ok(Value::Null);
            }
        }
        let v = eval_bound(&sel.select_exp, dsn, ctx)?;
        {
            let set = ctx.ds(dsn).expect("上面刚取到");
            set.selected[i] = Some(v.clone());
        }
        if let Some(di) = ctx.ds_index(dsn) {
            ctx.note_ds_row(di, i);
        }
        return Ok(ctx.ds(dsn).expect("同上").unwrap_row(i));
    }

    // ── Root 态（`:155-208`）──
    let n = {
        let set = ctx.ds(dsn).ok_or_else(|| missing(dsn))?;
        set.rows.len()
    };
    let mut idx: Vec<usize> = (0..n).collect();

    // filter（`Group.filter:201-263`）。⛔ 只实现**逐行 test** 路径；
    // `option` 含 `b`(98) 且条件是「本数据集列 == 外部值」的 Equals/And 树时
    // 产品走**二分**（`:232-237` → `:265-317`），前提是行已按该列有序
    // （spec-m5 §6.3 带账 ③：无序数据上的行为未探针）⇒ 不建模，逐行 test
    // 在有序数据上与之等价。
    if let Some(flt) = &sel.flt_exp {
        let saved = {
            let set = ctx.ds(dsn).expect("上面刚取到");
            set.current
        };
        let mut kept = Vec::new();
        for i in idx {
            {
                let set = ctx.ds(dsn).expect("上面刚取到");
                set.set_current(DsCur::Row(i));
            }
            let v = eval_bound(flt, dsn, ctx)?;
            if as_bool(&v)? {
                kept.push(i);
            }
        }
        ctx.ds(dsn).expect("同上").set_current(saved);
        idx = kept;
    }

    // distinct（`Group.distinct:803-826`）：按 distinctExp 的值去重，
    // **首现者胜出**（HashMap put 返回非 null 即跳过）；键相等 = [`Value`]
    // 相等（Java HashMap equals 同构：跨数值类型不等）。
    if let Some(d) = &sel.distinct_exp {
        let saved = {
            let set = ctx.ds(dsn).expect("上面刚取到");
            set.current
        };
        let mut seen: Vec<Value> = Vec::new();
        let mut kept = Vec::new();
        for i in idx {
            {
                let set = ctx.ds(dsn).expect("上面刚取到");
                set.set_current(DsCur::Row(i));
            }
            let k = eval_bound(d, dsn, ctx)?;
            if !seen.contains(&k) {
                seen.push(k);
                kept.push(i);
            }
        }
        ctx.ds(dsn).expect("同上").set_current(saved);
        idx = kept;
    }

    // `;` 多排序：先 sort 再 evaluateRows（`:161-163`）—— 与 `:` 次序的
    // 「先 evaluate 后 sort」是产品两条不同路径，刻意保留（模块文档警告 1）。
    if !sel.sort_exps.is_empty() {
        idx = multi_key_sort(&idx, &sel.sort_exps, &sel.is_descs, dsn, ctx)?;
    }

    // evaluateRows（`Group.evaluateRows:430-453`）
    let mut vals = eval_rows_idx(dsn, &idx, &sel.select_exp, ctx)?;

    // `:` 次序：evaluate 之后按**选列值**稳定排序（`:166-170` →
    // `SortDsValue.sort(list, desc):249-268`，`Collections.sort` 稳定）。
    if sel.sort_exps.is_empty() && sel.order != 0 {
        let asc = sel.order > 0;
        let mut pairs: Vec<(usize, Value)> = idx.into_iter().zip(vals).collect();
        pairs.sort_by(|a, b| {
            let o =
                crate::expr::num::aggregate_cmp(&a.1, &b.1).unwrap_or(std::cmp::Ordering::Equal);
            if asc {
                o
            } else {
                o.reverse()
            }
        });
        idx = pairs.iter().map(|(i, _)| *i).collect();
        vals = pairs.into_iter().map(|(_, v)| v).collect();
    }

    // size / num 逻辑（`:172-208`）
    let size = idx.len();
    if size == 0 {
        // `getNullRow()`：空 List 在格侧只清值（`cell_eval/extend.rs:100-103`），
        // 无 DsValue 身份可钉 ⇒ 不记旁路。
        return Ok(Value::Null);
    }
    if size == 1 || sel.num == 1 {
        // 单行（`:177-182`）：`z` 取末行。产品返回 **Row 对象** ⇒ 记行旁路，
        // 格侧走标量分支钉 `_$4`。
        let k = if rev { size - 1 } else { 0 };
        if let Some(di) = ctx.ds_index(dsn) {
            ctx.note_ds_row(di, idx[k]);
        }
        return Ok(vals[k].clone());
    }
    let (mut f_idx, mut f_vals) = if sel.num == -1 || (sel.num as usize) >= size {
        (idx, vals)
    } else if rev {
        // 末 num 行（`:200-208` 的 seq 从 size-1 递减收集）—— 先切片再统一倒序
        let num = sel.num as usize;
        (idx[size - num..].to_vec(), vals[size - num..].to_vec())
    } else {
        let num = sel.num as usize;
        (idx[..num].to_vec(), vals[..num].to_vec())
    };
    if rev {
        f_idx.reverse();
        f_vals.reverse();
    }
    if let Some(di) = ctx.ds_index(dsn) {
        ctx.note_ds_list(di, &f_idx);
    }
    Ok(Value::List(f_vals))
}
