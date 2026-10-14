//! DS 函数与 List 聚合（自 `expr/eval.rs` 拆出）。
//!
//! 涵盖：`ds.col`（DSVariable 求值）、`sum(...)` 的 List 聚合、
//! `ds.select/sum/count/avg/max/min(...)` 双态分派、`evaluateRows` 逐行循环。
//! 事实源：附件 H.2（数据集双态）；`DSSelect.java:141-205`、
//! `DSSum.java:22-63`、`Average.java:100-125`、`Max.java:100-121`、
//! `Min.java:100-121`、`Row.java:97-110`、`Sum.java:117-133`。
//!
//! ## M2-12（2026-10-13 落地）：`avg` / `max` / `min`
//!
//! 三个**纯函数**形态在 [`super::eval::evaluate_call`] 分发、**DS** 形态
//! （`ds.avg(col)`）在本文件的 [`ds_call`] 分发，两路共用同一批 List 聚合。
//! 两条已知边界（都显式报错，不静默）：
//!
//! * `Text-Text` 的字典序需要 `ChineseComparator` 的 `rule.txt`（不在仓库）；
//! * `avg`/`max`/`min` 的**非叶子参数**（`avg(A:B)` 区域形态）未建模 ——
//!   产品该路走 `_$1(ctx)` 逐格循环，与 `sum(A:B)` 同类。
//!
//! 循环依赖说明：`ds_call`/`eval_ds_arg` 要回调 [`super::eval::evaluate`]，
//! 而 `eval.rs` 的分发又要进来调 [`ds_call`] —— Rust 模块间互指是合法的，
//! 与产品里 `Expression.calculate ↔ DSSelect.calculate` 的互相调用同构。

use crate::error::{EngineError, Result};
use crate::expr::ast::{Call, Expr};
use crate::value::{single_value, Value};

use super::ds::DsCur;
use super::eval::EvalCtx;
use super::num::{aggregate_cmp, num_add, num_div};

/// `ds.col`（DSVariable）：Row 态取当前行该列；Root 态产品的 Group 语义
/// 未核实且 12 夹具不用 → `UnsupportedExpr`（§13.5 同类，诚实不支持）。
pub(crate) fn ds_column(ctx: &mut dyn EvalCtx, ds: &str, col: &str) -> Result<Value> {
    // 借用期收缩：`ctx.ds()` 的 &mut 不能跨后续调用存活
    let idx = {
        let set = ctx.ds(ds).ok_or_else(|| EngineError::MissingDataset {
            name: ds.to_string(),
        })?;
        set.col_index(col).ok_or_else(|| EngineError::UnknownCell {
            id: format!("{ds}.{col}"),
        })?
    };
    let set = ctx.ds(ds).expect("上面刚取到");
    Ok(match set.current {
        DsCur::Row(i) => set
            .rows
            .get(i)
            .and_then(|r| r.get(idx))
            .cloned()
            .unwrap_or(Value::Null),
        DsCur::Root => {
            return Err(EngineError::UnsupportedExpr {
                expr: format!("{ds}.{col} (Root)"),
            })
        }
    })
}

/// `Sum.calculate` 的 List 聚合（`Sum.java:117-133`）：跳 null、`Variant2.add`
/// 累加、全空 → `Double(0.0)`。
///
/// §M2-12 修正（2026-10-13）：原先只跳 `Value::Null`，**没过
/// `getSingleValue`** 那一层 ⇒ `sum(List[List(1)])` 会走到
/// `num_add(List, …)` 报 `UnsupportedExpr`，而产品返回 `1`。现已统一走
/// [`crate::value::single_value`]（与 `merge::same_value` 同一份实现）。
pub(crate) fn sum_list(list: Vec<Value>) -> Result<Value> {
    let mut acc: Option<Value> = None;
    for o in list {
        let Some(o) = single_value(&o) else { continue };
        acc = Some(match acc {
            None => o.clone(),
            Some(a) => num_add(&a, o)?,
        });
    }
    Ok(acc.unwrap_or(Value::Double(0.0)))
}

/// `Average.calculate` 的 List 聚合（`Average.java:100-125`）。
///
/// 与 [`sum_list`] 的三处差别，**全部**来自源码，不许"顺手统一"：
///
/// | # | 形态 | 结果 | 出处 |
/// |---|------|------|------|
/// | 1 | **空表** | `Null`（**不是** `Double(0.0)`） | `:112-114` `count == 0 → return null` |
/// | 2 | **全空元素** | `Double(0.0)` | `:122-124` |
/// | 3 | 分母 | `bContainNull ? count : count - nullCount` | `:125` |
///
/// `contain_null` = 调用带 `@a` 选项（`option.indexOf(97) != -1`，`:108`）——
/// 该选项**把 null 也计入分母**，用于 Excel 公式导出口径。
///
/// ⚠ 第 2 条必须先于除法判定，否则全空表会除零：`result == null` 时
/// `count - nullCount` 也恰为 0。
pub(crate) fn avg_list(list: &[Value], contain_null: bool) -> Result<Value> {
    if list.is_empty() {
        return Ok(Value::Null); // ① 空表 → null
    }
    let count = list.len();
    let mut acc: Option<Value> = None;
    let mut null_count = 0usize;
    for o in list {
        let Some(o) = single_value(o) else {
            null_count += 1;
            continue;
        };
        acc = Some(match acc {
            None => o.clone(),
            Some(a) => num_add(&a, o)?,
        });
    }
    let Some(acc) = acc else {
        return Ok(Value::Double(0.0)); // ② 全空元素 → Double(0.0)（先于除零）
    };
    let denom = if contain_null {
        count
    } else {
        count - null_count
    };
    num_div(&acc, &Value::Int(denom as i32)) // ③ `Variant2.divide` 无整型分支 ⇒ Double
}

/// `Max.calculate` 的 List 聚合（`Max.java:100-121`）：跳 null，`compare2` 取大。
///
/// ⛔ 与 [`min_list`] 的**不对称**是产品原文，不是笔误：
/// Max 用 `result = o`（已解包的标量），Min 却用 `result = list.get(i)`
/// （**未解包的原始元素**）—— 见 [`min_list`] 的说明。
///
/// 空表 / 全空元素 → `Null`（`result` 初值 null 且从未被赋值）。
pub(crate) fn max_list(list: &[Value]) -> Result<Value> {
    let mut acc: Option<Value> = None;
    for o in list {
        let Some(o) = single_value(o) else { continue };
        acc = Some(match acc {
            None => o.clone(),
            Some(a) => {
                // `if (compare2(result, o) >= 0) continue;`（`:113-114`）⇒ 保留较大者
                if aggregate_cmp(&a, o)? != std::cmp::Ordering::Less {
                    a
                } else {
                    o.clone()
                }
            }
        });
    }
    Ok(acc.unwrap_or(Value::Null))
}

/// `Min.calculate` 的 List 聚合（`Min.java:100-121`）：跳 null，`compare2` 取小。
///
/// ⛔ **产品原文的不对称，逐字复刻**：Max 写 `result = o`（解包后的标量），
/// 而 Min 写 `result = list.get(i)`（**未解包的原始 List 元素**）。
/// 两者只在"元素本身是**非空 `List`**"时分叉 —— 那种情况下
/// `getSingleValue` 会取首元素，而 Min 把整个 List 留下。
/// ⇒ `min(List[Int(1)], List[List(5), List(3)])` 在 Java 侧返回 `List[5]`。
/// 已在 `tests_eval.rs` 用单测钉住；**修平它之前先确认产品侧证据。**
///
/// 另：Min 调的是 `Variant2.compare`（`:363-367`）而 Max 调 `compare2`
/// —— 前者只是先过一次 `getSingleValue` 再转调 `compare2`，等价。
pub(crate) fn min_list(list: &[Value]) -> Result<Value> {
    let mut acc_idx: Option<usize> = None;
    for (i, o) in list.iter().enumerate() {
        let Some(unwrapped) = single_value(o) else {
            continue;
        };
        if let Some(k) = acc_idx {
            let cur = single_value(&list[k]).expect("acc_idx 指向已判定非空元素");
            // `if (compare2(result, o) <= 0) continue;`（`:113-114`）⇒ 保留较小者
            if aggregate_cmp(cur, unwrapped)? != std::cmp::Ordering::Greater {
                continue;
            }
        }
        acc_idx = Some(i);
    }
    // ⛔ `result = list.get(i)` —— 用**原始元素**，不是解包后的标量（`:115`）。
    Ok(acc_idx.map_or(Value::Null, |k| list[k].clone()))
}

/// DS 调用参数的求值：裸列名（`Unknown`）绑定到**本调用**的数据集
/// （`UnknownSymbol.setDS` 的语义）；其余表达式原样求值。
pub(crate) fn eval_ds_arg(arg: &Expr, dsn: &str, ctx: &mut dyn EvalCtx) -> Result<Value> {
    match arg {
        Expr::Unknown(col) => ds_current_data(ctx, dsn, col),
        other => super::eval::evaluate(other, ctx),
    }
}

/// `DataSet.getCurrentData(colNo)`：当前 DsValue 的该列数据。
/// Row 态 → 当前行该列；Root 态 → 不支持（同 [`ds_column`]）。
pub(crate) fn ds_current_data(ctx: &mut dyn EvalCtx, dsn: &str, col: &str) -> Result<Value> {
    let idx = {
        let set = ctx.ds(dsn).ok_or_else(|| EngineError::MissingDataset {
            name: dsn.to_string(),
        })?;
        set.col_index(col).ok_or_else(|| EngineError::UnknownCell {
            id: format!("{dsn}.{col}"),
        })?
    };
    let set = ctx.ds(dsn).expect("上面刚取到");
    Ok(match set.current {
        DsCur::Row(i) => set
            .rows
            .get(i)
            .and_then(|r| r.get(idx))
            .cloned()
            .unwrap_or(Value::Null),
        DsCur::Root => {
            return Err(EngineError::UnsupportedExpr {
                expr: format!("{dsn}.{col} (Root)"),
            })
        }
    })
}

/// DS 函数（`ds.名(...)`）：select / sum / count。
///
/// Row 态（数据集被扩展格引用，指针随实例推进）返回当前行的值；
/// Root 态做全量收集/聚合 —— 附件 H.2 的双态语义。
pub(crate) fn ds_call(c: &Call, dsn: &str, ctx: &mut dyn EvalCtx) -> Result<Value> {
    let is_row = {
        let set = ctx.ds(dsn).ok_or_else(|| EngineError::MissingDataset {
            name: dsn.to_string(),
        })?;
        matches!(set.current, DsCur::Row(_))
    };
    match (c.name.as_str(), is_row, c.args.len()) {
        // DSSelect Row 态（`DSSelect.java:143-149`）：setValue + 返回 Row ⇒ 拆包缓存值
        ("select", true, 1) => {
            let v = eval_ds_arg(&c.args[0], dsn, ctx)?;
            let i = {
                let set = ctx.ds(dsn).expect("上面刚取到");
                let i = set.current_row().unwrap_or(0);
                set.selected[i] = Some(v.clone());
                i
            };
            // ⛔ 旁路（§5.4 建模缺口）：产品此路返回的是 **Row 对象本身**
            // （`return cur;`，`DSSelect.java:149`），`ExtExtendCell.calculate:403/461`
            // 靠 `result instanceof DsValue` 把 `_$4` 定到这一行。Rust 的 [`Value`]
            // 是封闭九类、不携带身份 ⇒ 用 [`EvalCtx::note_ds_row`] 补记，
            // 由 `calc::cell_eval` 在求值后消费。
            if let Some(idx) = ctx.ds_index(dsn) {
                ctx.note_ds_row(idx, i);
            }
            Ok(ctx.ds(dsn).expect("同上").unwrap_row(i))
        }
        // DSSelect Root 态：全量 evaluateRows → List（`DSSelect.java:186-190`）
        ("select", false, 1) => {
            let rows = eval_rows(dsn, &c.args[0], ctx)?;
            // ⛔ 旁路：产品 Root 态返回 **List<Row>** —— `Group.getRows()`
            // （`Group.java:455-457`）返回的 `_$3` 元素全是 Row 对象
            // （`Group.evaluateRows:436-447` 逐元素 `(Row)this._$3.get(i)`），
            // 扩展格靠它逐元素写 `_$4`（`ExtExtendCell.calculate:401-407`）。
            // `eval_rows` 无 filter/distinct/sort ⇒ 元素 i 恒对应第 i 行。
            if let Some(idx) = ctx.ds_index(dsn) {
                let all: Vec<usize> = (0..rows.len()).collect();
                ctx.note_ds_list(idx, &all);
            }
            Ok(Value::List(rows))
        }
        // DSSum Row 态（`DSSum.java:41-47`）：同 select 的 Row 路径
        ("sum", true, 1) => {
            let v = eval_ds_arg(&c.args[0], dsn, ctx)?;
            let i = {
                let set = ctx.ds(dsn).expect("上面刚取到");
                let i = set.current_row().unwrap_or(0);
                set.selected[i] = Some(v.clone());
                i
            };
            // ⛔ 旁路：同 select Row 态（`DSSum.java:46` 同样 `return cur;`）。
            if let Some(idx) = ctx.ds_index(dsn) {
                ctx.note_ds_row(idx, i);
            }
            Ok(ctx.ds(dsn).expect("同上").unwrap_row(i))
        }
        // DSSum Root 态：全量和（`DSSum.java:50-61`）。⛔ 返回**标量**
        // （聚合值或 `Double(0.0)`），不是 Row 也不是 List ⇒ 不记旁路。
        ("sum", false, 1) => sum_list(eval_rows(dsn, &c.args[0], ctx)?),
        // ── M2-12：avg / max / min。Row 态与 sum 同形（`return cur;`），
        //    Root 态各自聚合（`Average.calculate:100-125` / `Max:100-121` /
        //    `Min:100-121`；三个 DS 包装类在 reportlite 树里**不存在**
        //    ⇒ 与 sum 共用 DSSum 的 `return cur;` 形态，取 row 态同一返回值）。
        ("avg" | "max" | "min", true, 1) => {
            let v = eval_ds_arg(&c.args[0], dsn, ctx)?;
            let i = {
                let set = ctx.ds(dsn).expect("上面刚取到");
                let i = set.current_row().unwrap_or(0);
                set.selected[i] = Some(v.clone());
                i
            };
            // ⛔ 旁路：同 sum 的 Row 态（产品这三个函数也返回当前 Row）。
            if let Some(idx) = ctx.ds_index(dsn) {
                ctx.note_ds_row(idx, i);
            }
            Ok(ctx.ds(dsn).expect("同上").unwrap_row(i))
        }
        ("avg", false, 1) => {
            let rows = eval_rows(dsn, &c.args[0], ctx)?;
            let a = c.option.as_deref().unwrap_or("").contains('a');
            avg_list(&rows, a)
        }
        ("max", false, 1) => max_list(&eval_rows(dsn, &c.args[0], ctx)?),
        ("min", false, 1) => min_list(&eval_rows(dsn, &c.args[0], ctx)?),
        // DSCount Root 态：行数（夹具 ds2.count() → 3）
        ("count", false, 0) => {
            let set = ctx.ds(dsn).expect("上面刚取到");
            Ok(Value::Int(set.rows.len() as i32))
        }
        (name, _, n) => Err(EngineError::UnsupportedExpr {
            expr: format!("{dsn}.{name} 该形态未支持（{n} 参数）"),
        }),
    }
}

/// `Group.evaluateRows`：逐行把 ds 指针切到该行求值（`Row.java:97-110` 的循环
/// 形态；`Row.evaluate` 的"保存 → setCurrent → finally 恢复"展开成外层循环）。
pub(crate) fn eval_rows(dsn: &str, col_expr: &Expr, ctx: &mut dyn EvalCtx) -> Result<Vec<Value>> {
    let (n, saved) = {
        let set = ctx.ds(dsn).ok_or_else(|| EngineError::MissingDataset {
            name: dsn.to_string(),
        })?;
        (set.rows.len(), set.current)
    };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        {
            let set = ctx.ds(dsn).expect("上面刚取到");
            set.set_current(DsCur::Row(i));
        }
        out.push(eval_ds_arg(col_expr, dsn, ctx)?);
    }
    ctx.ds(dsn).expect("上面刚取到").set_current(saved);
    Ok(out)
}
