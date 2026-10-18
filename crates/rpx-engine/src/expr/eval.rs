//! 求值器：AST → `Value`（§12.1 第 9 步切片）。
//!
//! Java 对应：`Expression.calculate`（逐节点 `calculate(ctx)`）+ 各函数类。
//! 附件 H.1/H.2 是求值语义的事实源（⛔ **不是 Excel 语义**）。
//!
//! 文件按职责拆分：本文件只留 [`EvalCtx`] trait、分发入口 [`evaluate`]、
//! 函数/一元/二元分发与区域处理器（[`sum_link`] / [`region_aggregate`]）；
//! DS 函数与 List 聚合在 [`dsfn`]，数值原语（`Variant2.*` 移植）在 [`num`]，
//! 区域两端点的收集（`[A:B]` / `(A:B)` → `CellSet`）在 [`super::cellset`]。
//!
//! # 本切片已实现（12 夹具的完整表达式面，`EngineModels.java` 全量清单）
//!
//! | 形态 | 语义 | 出处 |
//! |------|------|------|
//! | `B2` / `$B2` | CSVariable 相对引用（`resolve.rs`） | `CSVariable.java` |
//! | `B2*2` / `6*1` / … | 算术，**类型保真**（Int 不提升） | 附件 H.3 |
//! | `ds1.select(col)` | Row 态：当前行值；Root 态：整列 List | `DSSelect.java:141-205` |
//! | `ds1.sum(col)` | Row 态：当前行值；Root 态：全量和 | `DSSum.java:22-63` |
//! | `ds1.count()` | Root 态：行数 | `DSCount` |
//! | `sum(x)` | List → 聚合；标量 → 透传；空 → `Double(0.0)` | `Sum.java:106-133` |
//! | `sum(A:B)` | ⛔ **不是区域求和**：把 B 设为 current 后求值 A 一次 | `Sum.java:174-206` |
//! | `sum(@:[B2:B4])` | List 逐格：逐格 `setCurrent` 后求值 A 再聚合（尾 4 行同） | `Sum.java:149-172` |
//!
//! # 求值上下文 [`EvalCtx`]
//!
//! 产品里这些能力散在 `ExtCellSet`（current 两指针、`getSource`）与格对象
//! （head 链、实例数组）上；这里收拢为一个 trait，让单测可以注入假实现，
//! 下一接线步由 `HeadState` 实现生产版。CSVariable 的解析算法在
//! [`super::resolve`] 里以 `&dyn EvalCtx` 为参数 —— 不依赖具体网格类型。
//!
//! # 已知边界（诚实声明）
//!
//! - `group`/`sort`/`Move`/`Offset`/`Select{}` 的 `left` 语义、其它区间运算与
//!   `In`/`Eval` → `UnsupportedExpr`；
//! - `sum/avg/max/min(A:B)` 的**单格 else 分支**（`sum(B2:B4)=100`）与
//!   **List 逐格分支**（`sum(@:[B2:B4])=420`）均已实现，共用 [`region_loop`]
//!   的骨架，操作数角色不对称（avg 反转）见 [`region_aggregate`] 文档。
//! - `firstLeft` 序列化门、`option 'r'`、`num/distinct` 参数未建模（夹具不用）。

use crate::arena::CellId;
use crate::error::{EngineError, Result};
use crate::expr::ast::{Call, Expr};
use crate::value::{single_value, Value};

use super::cellset::{eval_cellset, CellSet};
use super::ds::DataSet;
use super::dsfn::{avg_list, ds_call, ds_column, max_list, min_list, sum_list};
// ⛔ 运算符的数值原语（`as_bool` / `num_neg` / `val_eq` …）已随
//    `evaluate_unary`/`evaluate_binary` 一起搬进 `ops`，这里只留区域聚合要用的三个。
use super::num::{aggregate_cmp, num_add, num_div};

mod barcode;
mod date;
/// 一元 / 二元运算的求值（拆出只为门禁行数 G4，见该模块文档）。
mod ops;
pub(crate) mod scope;
mod tutorial;
use ops::{evaluate_binary, evaluate_unary};

/// 「空行」哨兵 —— [`EvalCtx::note_ds_row`] 的 `row` 参数专用值。
///
/// M5-01：`ds.select` 在 Row 态**过滤未命中**时产品返回 `getNullRow()`
/// （`DSSelect.java:153`），格靠 `result instanceof DsValue` 把 `_$4` 钉成
/// **空行身份**（`ExtExtendCell.calculate:461`）⇒ 本仓用
/// `note_ds_row(ds, NULL_ROW_NOTE)` 表达同一件事（calc 侧
/// `DsInstance::is_real_row` / `mirror` 已按哨兵分流，`types.rs` 有编译期
/// 断言把两边钉死同值）。
pub const NULL_ROW_NOTE: usize = usize::MAX;

/// 求值上下文（见模块文档）。
///
/// DS 值归属旁路的 trait（见 `docs/m2-engine-design.md` §5.4 的建模缺口）。
pub trait EvalCtx {
    // ── head 图原语（CSVariable 解析所需；`ExtCell` 的可观察面）──
    /// 实例的引擎出处（`getSource()`；引擎矩阵本体格返回自身）。
    fn source_of(&self, c: CellId) -> CellId;
    fn left_head(&self, c: CellId) -> CellId;
    fn top_head(&self, c: CellId) -> CellId;
    /// 左/上子格列表（`_$6`/`_$5`；仅扩展格与 root 非空）。
    fn left_subs(&self, c: CellId) -> Vec<CellId>;
    fn top_subs(&self, c: CellId) -> Vec<CellId>;
    /// 扩展实例数组（`_$7`；非扩展格为空）。
    fn ext_cells(&self, c: CellId) -> Vec<CellId>;
    /// 生效 extensible（`getExtensible()`）。
    fn extensible(&self, c: CellId) -> i8;
    /// `cell00`。
    fn root(&self) -> CellId;
    // ── current 两指针与取值（`ExtCellSet.java:578-596`）──
    /// `getCurrent()`（`_$21`）：**当前正在求值的格**。相对引用的基准。
    fn current(&self) -> CellId;
    /// `getSavedCurrent()`（`_$20`）：`saveCurrent()` 存下的那一份。`$B2` 用的是它。
    fn saved_current(&self) -> CellId;
    /// `setCurrent(c)`（`:578-580`）：只写 `_$21`。
    fn set_current(&mut self, c: CellId);
    /// `saveCurrent()`（`:586-588`）：`_$20 = _$21`。
    ///
    /// ⛔ 只有 `ExtNormalCell.calculate:647` 与 `Sum` 的 cellSet 分支（`Sum.java:150`）
    /// 会调它 —— 不要"顺手"在别处补，`_$20` 被覆盖会让外层 `resumeCurrent` 回错格。
    fn save_current(&mut self);
    /// `resumeCurrent()`（`:594-596`）：`_$21 = _$20`。
    fn resume_current(&mut self);
    /// 实例当前值（`getValue(false)`；未算 → `Null`）。
    fn cell_value(&self, c: CellId) -> Value;
    /// 实例当前值，**先过 `testValue()` 门**（`ExtNormalCell.getValue:326-336`）。
    ///
    /// ⛔ 产品的 `getValue(boolean)` **无条件调 `this.testValue()`**（`:334`）——
    /// 未求值的格被引用 ⇒ 置 bit31 并抛挂起（依赖驱动的 chase 求值）；
    /// 第二次被引用 ⇒ state 1（CIRCLE，循环引用检测的机制本体）。
    /// [`Self::cell_value`] 的裸读形态只是给**假实现**与 `resolve` 内部用的；
    /// 引用取值路径（`Expr::CellRef`）必须走本方法。
    /// 默认实现 = 裸读（不建模依赖检测）；生产版由 `CalcCtx` 实现。
    fn cell_value_checked(&mut self, c: CellId) -> Result<Value> {
        Ok(self.cell_value(c))
    }
    /// `getSource(id)`：按格号查**引擎矩阵**；不存在 → `None`。
    fn source_by_id(&self, id: &str) -> Option<CellId>;
    // ── 计算态矩阵与几何（区域遍历 [`super::cellset`] 所需）──
    /// `ExtCellSet.getCell(row, col)`：计算态坐标（`_$26`）→ 格 id。
    ///
    /// ⛔ 与 [`Self::source_by_id`] 查的是**两张不同的矩阵**：这个查计算矩阵
    /// `_$26`（扩展后、含实例格），那个查引擎矩阵 `_$25`（模板态）。
    /// 矩形遍历（`Link._$1:83`）读的是前者。
    fn cell_at(&self, _row: i32, _col: i32) -> Option<CellId> {
        None
    }
    /// 格的**自身**计算态坐标（`ExtCell.getRow()/getCol()`）。
    ///
    /// 与 [`Self::cell_at`] 的入参可能**不相等**——`_$26` 里的合并延续槽是
    /// 指向锚点的别名，遍历靠"自身坐标 == 遍历坐标"把它排除
    /// （`Link.java:84` 的 `cell.getRow() != i || cell.getCol() != j`）。
    fn cell_pos(&self, _c: CellId) -> (i32, i32) {
        (0, 0)
    }
    /// `getRowMerge()`：合并跨度 —— 矩形**右端点**要加它才是 bottom（`Link.java:73`）。
    fn row_merge(&self, _c: CellId) -> i32 {
        1
    }
    /// `getColMerge()`：同 [`Self::row_merge`]，取 right（`Link.java:73`）。
    fn col_merge(&self, _c: CellId) -> i32 {
        1
    }
    /// 收集期 `cell.testValue()`（`Link.java:85` / `ValueList.java:116`）。
    /// 默认空实现（假实现不建模依赖检测）。见 [`super::cellset`] 模块文档。
    fn test_value_gate(&mut self, _c: CellId) -> Result<()> {
        Ok(())
    }
    // ── 数据集 ──
    //
    // ⛔ **没有求值期参数查询**（M3-01 删除）：参数值在建树期就内联成
    // [`Expr::Const`]（`params.rs` 模块文档），求值期不需要也不应该认识"参数"。
    fn ds(&mut self, name: &str) -> Option<&mut DataSet>;
    /// 数据集名 → 稳定下标（DS 旁路与 `DsCurrentHost` 都要用它寻址）。
    fn ds_index(&self, name: &str) -> Option<usize>;
    // ── DS 值归属旁路（见 `docs/m2-engine-design.md` §5.4 的建模缺口）──
    /// 记下「本次表达式求值的最终结果就是**第 `ds` 号数据集的第 `row` 行**」。
    ///
    /// 对应 Java 里 `ds.select(col)` / `ds.sum(col)` 在 **Row 态**返回 `DsValue`
    /// 对象本身（`DSSelect.java:145-149`），而 `ExtExtendCell.calculate:403/461`
    /// 靠 `result instanceof DsValue` 把 `_$4` 定位到行。Rust 的 [`Value`] 是
    /// **封闭的九类值**、不承载运行时对象 ⇒ 用这条旁路把那层"身份"补回来。
    fn note_ds_row(&mut self, _ds: usize, _row: usize) {}
    /// 记下「本次求值的最终结果是**第 `ds` 号数据集的全量行**，逐元素行号如 `rows`」。
    fn note_ds_list(&mut self, _ds: usize, _rows: &[usize]) {}
    /// 组身份与行身份共用同一条扩展旁路。
    fn note_ds_members(&mut self, _ds: usize, _members: &[super::ds::DsCur]) {}
    /// 每次逐格求值**开始前**清空旁路，这样只会留下"这一次求值"的记录。
    fn clear_ds_notes(&mut self) {}
    /// 取出本次求值记下的"单个行"归属（若有）。
    fn ds_row_note(&self) -> Option<(usize, super::ds::DsCur)> {
        None
    }
    /// 取出本次求值记下的"整列"归属（若有）。
    fn ds_list_note(&self) -> Option<(usize, Vec<super::ds::DsCur>)> {
        None
    }
}

/// 求值入口（`Expression.calculate(ctx)` 的 Rust 形态）。
pub fn evaluate(expr: &Expr, ctx: &mut dyn EvalCtx) -> Result<Value> {
    match expr {
        Expr::Const(v) => Ok(v.clone()),
        // CSVariable（`B2` / `$B2`）：解析到实例 → 过 testValue 门再取值
        // （`getValue(false):334` 无条件 testValue —— 见 `cell_value_checked` 文档）
        Expr::CellRef {
            id,
            absolute,
            provisional: _,
        } => {
            match super::resolve::resolve_cell(ctx, id, *absolute) {
                Some(cell) => ctx.cell_value_checked(cell),
                None => Ok(Value::Null), // Java getCell()==null ⇒ getValue 返回 null
            }
        }
        // CSCurrent（`@`）：当前格的值
        Expr::CurrentCell => Ok(ctx.cell_value(ctx.current())),
        // DSVariable（`ds1.qty`）：Row 态取当前行该列；Root 态按产品
        // `DataSet.getCurrentData:395-410` 的 currentRow 光标取首行值
        // （M5-01 核实并实现，见 `dsfn::root_current_data`）。
        Expr::DsColumn { ds, col } => ds_column(ctx, ds, col),
        // UnknownSymbol：DS 调用参数里的裸列名由该 DS 绑定数据集（`setDS` +
        // `ds.getCurrentData(colNo)`，`UnknownSymbol.java:32-37/48-51`，见
        // `dsfn::eval_ds_arg`）；独立出现的 Unknown 产品会抛 unknownExpression。
        Expr::Unknown(name) => Err(EngineError::UnsupportedExpr { expr: name.clone() }),
        // ValueList（`[...]` / `list(...)`）：逐项求值成 List
        Expr::ValueList { items, .. } => {
            let mut vs = Vec::with_capacity(items.len());
            for it in items {
                vs.push(evaluate(it, ctx)?);
            }
            Ok(Value::List(vs))
        }
        // Select{}：两种位置（见 `ast.rs` 的变体文档）。带目标的形态是**格集**
        // （`Value::List`，逐格取值），无目标的是普通前缀花括号。
        Expr::Select { target: None, arg } => evaluate(arg, ctx),
        Expr::Select {
            target: Some(t),
            arg,
        } => super::cellset::eval_braced_set(t, arg, ctx),
        // `Cellx[...]` —— 层次坐标 / 位移坐标 / 根坐标（教程 `056`/`057`/`061`）。
        // 无 `{}` ⇒ **标量**：定位到多格取第一个（`056_bzbb121` 末段）。
        Expr::Coord { target, spec } => super::cellset::eval_coord_scalar(target, spec, ctx),
        // Move/Offset：非格号左操作数的后缀形态（如 `(A1)[2]`），未支持
        Expr::Index { .. } => Err(EngineError::UnsupportedExpr {
            expr: "Move/Offset".into(),
        }),
        // `ds[i].列`（DSOffset + DSMember；探针 ProbeDsOffset O1–O9 实证）
        Expr::DsOffsetColumn { ds, offset, col } => {
            super::dsfn::ds_offset_column(ctx, ds, offset, col)
        }
        Expr::Call(c) => evaluate_call(c, ctx),
        Expr::Unary { op, rhs } => evaluate_unary(*op, rhs, ctx),
        Expr::Binary { op, lhs, rhs } => evaluate_binary(*op, lhs, rhs, ctx),
    }
}

fn evaluate_call(c: &Call, ctx: &mut dyn EvalCtx) -> Result<Value> {
    // ── DS 函数（`ds.名(...)`）──
    if let Some(dsn) = &c.qualifier {
        return ds_call(c, dsn, ctx);
    }
    match c.name.as_str() {
        "year" | "month" => date::call(c, ctx),
        "count" | "to" | "if" | "ifn" | "int" | "seq" | "value" => tutorial::call(c, ctx),
        "barcode" => barcode::call(c, ctx),
        "sum" if c.args.len() == 2 => sum_link(c, ctx),
        // ── sum：叶子 / 复合两路（`Sum.java:106-206`）──
        // ── M2-12：avg / max / min 的 `A:B` 区域形态（单格 else 分支）──
        "avg" | "max" | "min"
            if c.param_kind == crate::expr::ast::ParamKind::Colon && c.args.len() == 2 =>
        {
            region_aggregate(c, ctx)
        }
        "sum" if c.args.len() == 1 => {
            let v = evaluate(&c.args[0], ctx)?;
            if v.is_null() {
                return Ok(Value::Double(0.0)); // `result1 == null → Double(0.0)`（`:111-113`）
            }
            match v {
                Value::List(l) => sum_list(l),
                scalar => Ok(scalar), // 标量透传（`:115-117`）
            }
        }
        // ── M2-12：avg / max / min（`Average.java:100-125` / `Max:100-121` /
        //    `Min:100-121`）。三者的叶子路径同构：标量透传、List 才聚合。
        "avg" if c.args.len() == 1 => {
            let v = evaluate(&c.args[0], ctx)?;
            match v {
                Value::List(l) => avg_list(&l, c.option.as_deref().unwrap_or("").contains('a')),
                scalar => Ok(scalar), // `else { return result1; }`（`:120-122`）
            }
        }
        "max" if c.args.len() == 1 => {
            let v = evaluate(&c.args[0], ctx)?;
            match v {
                Value::List(l) => max_list(&l),
                scalar => Ok(scalar), // `else { result = result1; }`（`:115-117`）
            }
        }
        "min" if c.args.len() == 1 => {
            let v = evaluate(&c.args[0], ctx)?;
            match v {
                Value::List(l) => min_list(&l),
                scalar => Ok(scalar), // 同 max
            }
        }
        other => Err(EngineError::UnsupportedExpr {
            expr: format!("{other}(...)"),
        }),
    }
}

/// `sum(A:B)` 的区域入口（`Sum.java:128-144`）：`cellSet = sub1`（args[1]），
/// 求值 `sub0`（args[0]）。
///
/// 累积与收尾见 [`region_loop`]：`result == null → Double(0.0)`（`:192-195`）。
fn sum_link(c: &Call, ctx: &mut dyn EvalCtx) -> Result<Value> {
    let op = require_cellset(c, 1, ctx)?;
    region_loop(&op, &c.args[0], RegionMode::Sum, ctx)
}

/// `avg/max/min(A:B)` 的区域入口。
///
/// **操作数角色：avg 与 sum/max/min 相反**（`engine_avgrange` 夹具定案，
/// B2=100,B3=200,B4=120，产品实跑值）：
/// - `sum/max/min(A:B)`：`cellSet = sub1`（args[1]），求值 `sub0`（args[0]）
///   ⇒ 返回**操作数1**。实测 `sum/max/min(B2:B4)` 全 = `100`（=B2）。
/// - `avg(A:B)`：`cellSet = sub0`（args[0]），求值 `sub1`（args[1]）
///   ⇒ 返回**操作数2**，且 `Variant2.divide(_, 1)` 恒升 Double。
///   实测 `avg(B2:B4)` = `Double(120)`（=B4），⛔ 不是均值 140。
///   （`Average.java:139-149` 的 `cellSet=sub0` 与 Sum/Max/Min 的 `cellSet=sub1`
///   是**原文的不对称**，见 `docs/m2-engine-design.md` §14。）
///
/// 单格与 List 两种 cellSet **共用** [`region_loop`] 的同一条骨架：`B2:B4` 落
/// 单格（`sum(B2:B4)=100`），`[B2:B4]` / `(B2:B4)` 落 List 逐格
/// （`sum(@:[B2:B4])=420`、`avg([B2:B4]:@)=140.0`）—— 第 15 个产品夹具
/// `engine_listrange` 把两种形态钉在一起，`docs/m2-engine-design.md` §14.6。
fn region_aggregate(c: &Call, ctx: &mut dyn EvalCtx) -> Result<Value> {
    let is_avg = c.name == "avg";
    let (cellset_idx, value_idx) = if is_avg { (0, 1) } else { (1, 0) };
    let op = require_cellset(c, cellset_idx, ctx)?;
    let mode = match c.name.as_str() {
        // `option.indexOf(97) != -1`（`Average.java:106`）：'a' 选项把 null 计入分母
        "avg" => RegionMode::Avg {
            contain_null: c.option.as_deref().unwrap_or("").contains('a'),
        },
        "max" => RegionMode::Max,
        _ => RegionMode::Min,
    };
    region_loop(&op, &c.args[value_idx], mode, ctx)
}

/// 取第 `idx` 个操作数并求值成 [`CellSet`]；形态不认识 ⇒ `UnsupportedExpr`
/// （产品在同样位置抛 `function.paramTypeError`，`Sum.java:165-166` 等）。
fn require_cellset(c: &Call, idx: usize, ctx: &mut dyn EvalCtx) -> Result<CellSet> {
    let Some(arg) = c.args.get(idx) else {
        return Err(EngineError::UnsupportedExpr {
            expr: format!("{} 缺少第 {} 个操作数", c.name, idx + 1),
        });
    };
    eval_cellset(arg, ctx)?.ok_or_else(|| EngineError::UnsupportedExpr {
        expr: format!("{} cellSet 形态未支持: {arg:?}", c.name),
    })
}

/// 区域处理器的累积模式（四个函数的骨架相同，只有累积与收尾不同）。
#[derive(Debug, Clone, Copy)]
enum RegionMode {
    /// `Sum.java:155-196`：`Variant2.add` 累加；收尾 `result == null → Double(0.0)`。
    Sum,
    /// `Max.java:136-185`：`Variant2.compare` 取大；收尾原样（可能是 `null`）。
    Max,
    /// `Min.java:136-185`：同 `Max`，取小。
    Min,
    /// `Average.java:155-207`：`add` 累加 + 记 `nullCount`；
    /// 收尾 `divide(result, contain_null ? count : count - nullCount)`。
    Avg { contain_null: bool },
}

/// 区域处理器的公共骨架：`saveCurrent()` → 逐格 `setCurrent` + 求值 `param2`
/// → `finally` 语义的 `resumeCurrent()`（求值失败也回卷）。
///
/// ⛔ **Single 与 Many 共用同一条循环**，因为产品的 else 分支
/// （`Sum.java:173-187` 等）与 List 分支的**单轮**迭代逐字等价：`result =
/// Variant2.getSingleValue(obj)` 正是循环首轮（`result == null`）走的那一支。
/// 唯一差别是 avg 的分母 —— List 分支 `count = list.size()`、else 分支
/// `count = 1`，而单格时 `cells.len()` 恰为 1。
///
/// ⛔ 三条不能"顺手统一"的原文差异：
/// 1. `add` 的右操作数**不过** `getSingleValue`（`Sum.java:171` 的 `obj` 是原始
///    对象），而 `calculate` 的 List 分支是 `add(result, getSingleValue(...))`
///    （`:118`）—— 两条路不是同一份代码；
/// 2. `Max/Min` 用 `Variant2.compare`（**两侧都解包**，`Variant2.java:364-368`），
///    不是 `compare2`；
/// 3. avg 的 `nullCount` 只在 List 分支存在，但单格分支因 `result == null`
///    提前返回而**用不到**分母 ⇒ 共用骨架仍等价。
fn region_loop(
    op: &CellSet,
    value_expr: &Expr,
    mode: RegionMode,
    ctx: &mut dyn EvalCtx,
) -> Result<Value> {
    let cells: &[CellId] = match op {
        CellSet::Single(c) => std::slice::from_ref(c),
        CellSet::Many(v) => v,
    };
    scope::with_current(ctx, |ctx| region_accumulate(cells, value_expr, mode, ctx))
}

/// [`region_loop`] 的循环体与收尾（拆出来只为让 `save`/`resume` 的配对一眼可见）。
fn region_accumulate(
    cells: &[CellId],
    value_expr: &Expr,
    mode: RegionMode,
    ctx: &mut dyn EvalCtx,
) -> Result<Value> {
    let mut acc: Option<Value> = None;
    let mut null_count = 0i32;
    for &cell in cells {
        ctx.set_current(cell); // `ecs.setCurrent(cell)`
        let obj = evaluate(value_expr, ctx)?; // `param2.calculate(ctx)`
        if obj.is_null() {
            // sum/max/min：`if (obj == null) continue;`；avg：`++nullCount; continue;`
            if matches!(mode, RegionMode::Avg { .. }) {
                null_count += 1;
            }
            continue;
        }
        let next = match mode {
            RegionMode::Sum | RegionMode::Avg { .. } => match acc.take() {
                // 首轮 `Variant2.getSingleValue(obj)`（取到 null ⇒ 保持未累积）
                None => unwrap_single(&obj),
                // 后续轮 `Variant2.add(result, obj)`（⛔ obj 未解包）
                Some(a) => Some(num_add(&a, &obj)?),
            },
            RegionMode::Max | RegionMode::Min => match acc.take() {
                None => unwrap_single(&obj),
                Some(a) => {
                    let b = unwrap_single(&obj);
                    let bv = b.clone().unwrap_or(Value::Null);
                    let ord = aggregate_cmp(&a, &bv)?;
                    let take_b = match mode {
                        RegionMode::Max => ord == std::cmp::Ordering::Less, // `>= 0 → continue`
                        _ => ord == std::cmp::Ordering::Greater,            // `<= 0 → continue`
                    };
                    // 产品同处写 `result = Variant2.getSingleValue(obj)`（`Max.java:163` /
                    // `Min.java:163`）：取到 null 时 result 变 **Java null**，下一轮
                    // `result != null` 为假 ⇒ 走「无条件重新初始化」。`None` 正是该语义；
                    // ⛔ 不能回写成 `Some(Value::Null)` —— 那样下轮会走比较分支
                    // （`compare2(null, b) = -1`：Max 的 `>= 0 → continue` 判假、
                    // Min 的 `<= 0 → continue` 判真，两边都与产品分叉）。
                    // 可达条件：元素是「空 List 或首元素为 null 的 List」——
                    // 产品夹具无此形态，由合成钉子 `tests_aggregate.rs` ⑫ 覆盖。
                    if take_b {
                        b
                    } else {
                        Some(a)
                    }
                }
            },
        };
        acc = next;
    }
    match mode {
        // `Sum.java:192-195`
        RegionMode::Sum => Ok(acc.unwrap_or(Value::Double(0.0))),
        // `Max.java:184` / `Min.java:184`：原样返回（null 保持 null）
        RegionMode::Max | RegionMode::Min => Ok(acc.unwrap_or(Value::Null)),
        // `Average.java:203-206`
        RegionMode::Avg { contain_null } => {
            let Some(res) = acc else {
                return Ok(Value::Double(0.0)); // `result == null → Double(0.0)`（先于除零）
            };
            let count = cells.len() as i32;
            let denom = if contain_null {
                count
            } else {
                count - null_count
            };
            num_div(&res, &Value::Int(denom))
        }
    }
}

/// `Variant2.getSingleValue`（`Variant2.java:85-94`）的 Rust 形态，并把
/// "取到 null"（空 List 或首元素是 null）**归一成 `None`** —— Java 的
/// `result != null` 判的就是这个，`Option` 的 `None` 正好承载它。
fn unwrap_single(v: &Value) -> Option<Value> {
    match single_value(v) {
        None | Some(Value::Null) => None,
        Some(o) => Some(o.clone()),
    }
}
