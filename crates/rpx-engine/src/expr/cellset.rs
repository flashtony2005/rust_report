//! 区域操作数（`cellSet`）的求值 —— `sum/avg/max/min(A:B)` 的 **List 逐格分支**入口。
//!
//! # 为什么单列一层
//!
//! 产品的区域处理器（`Sum.java:149-196` / `Average.java:155-207` /
//! `Max.java:136-185` / `Min.java:136-185`）接到的 `Object cellSet` 可能是
//! **`CSVariable`**（单格 ⇒ else 分支）或 **`List<ExtCell>`**（格集合 ⇒ List
//! 逐格分支）。后者在九类值里**没有对应物**，所以本 crate 用 [`CellSet`]
//! 在求值器内部承载它，**不污染 [`crate::value::Value`]**。
//!
//! # 什么形态能求值成 `List<ExtCell>`（`ProbeListRange.java` 实证）
//!
//! | 形态 | Java | 说明 |
//! |------|------|------|
//! | `[B2:B4]` | `ValueList.java:161-162` → `:68-120` | `:` 形态对**计算态坐标**做矩形遍历 |
//! | `(B2:B4)` | `Link.java:55-90` | 括号被 `ParamParser.java:110-117` 保留 ⇒ 落在同一遍历上 |
//!
//! ⛔ 普通 `B2:B4` **永远到不了这里**：`ParamParser.java:66-79` 把 `:` 当参数
//! 分隔符拆成两个 sub，cellSet 于是求值成 `CSVariable`（走 else 分支）。
//! 这就是"`sum(B2:B4)`=100 而 `sum(@:(B2:B4))`=420"的由来（`engine_listrange`）。
//!
//! # 收集期的 `testValue()`
//!
//! 两条遍历对每个命中格调 `cell.testValue()`（`Link.java:85`、
//! `ValueList.java:116`）—— 依赖驱动的 chase 求值由此触发。Rust 侧经
//! [`EvalCtx::test_value_gate`] 转发到既有的 `cell_eval::test_value`，
//! 挂起与循环错误照常穿透。
//!
//! # 未覆盖（诚实声明）
//!
//! * `[B2]`（`ValueList` 的 `Leaf` 形态）与 `[a,b,c]`（逗号形态）：产品对
//!   单格项走 `CSVariable.getCells(null)`（取该格的**全部扩展实例**，
//!   `ValueList.java:130-133`），本 crate 未建模 —— **无夹具**。
//! * `Move`（`B2[...]` 后缀）与 `DSOffset`：产品另有实现，不在本路径（
//!   `Expression.java:136-155` 按 `preNode` 分类）。
//!
//! 算法正文见 `docs/m2-engine-design.md` §14.6。

use crate::arena::CellId;
use crate::error::{EngineError, Result};

use super::ast::{BinOp, Expr, ParamKind};
use super::eval::EvalCtx;
use super::resolve::resolve_cell;

/// `cellSet` 操作数的求值结果（对应产品的 `CSVariable` / `List<ExtCell>`）。
#[derive(Debug)]
pub(crate) enum CellSet {
    /// 单个格（`Sum.java:174-181` 的 else 分支）。
    Single(CellId),
    /// 格集合（`Sum.java:155-172` 的 List 分支），**按遍历顺序**。
    Many(Vec<CellId>),
}

/// 把一个 `cellSet` 操作数求值成 [`CellSet`]；形态不认识 → `None`
/// （调用方按"产品也会在这里抛 paramTypeError"报 `UnsupportedExpr`）。
pub(crate) fn eval_cellset(expr: &Expr, ctx: &mut dyn EvalCtx) -> Result<Option<CellSet>> {
    match expr {
        // `B4` / `$B4`：CSVariable ⇒ 单格（`Sum.java:135` 的 `sub1.calculate`）
        Expr::CellRef {
            id,
            absolute,
            provisional: _,
        } => Ok(resolve_cell(ctx, id, *absolute).map(CellSet::Single)),
        // `[B2:B4]`：ValueList 的 `:` 形态（`ValueList.java:161-162`）
        Expr::ValueList {
            kind: ParamKind::Colon,
            items,
        } if items.len() == 2 => Ok(Some(CellSet::Many(rect_cells(&items[0], &items[1], ctx)?))),
        // `(B2:B4)`：括号保留的 `:` 是 Link（`Link.java:55-90`）
        Expr::Binary {
            op: BinOp::Link,
            lhs,
            rhs,
        } => Ok(Some(CellSet::Many(rect_cells(lhs, rhs, ctx)?))),
        _ => Ok(None),
    }
}

/// 矩形遍历 —— `Link._$1`（`:78-89`）与 `ValueList._$1`（`:112-119`）的**共同语义**。
///
/// 两条循环逐字等价：
/// * `top/left` 取**左端格自身**坐标（`cr1.getBeginRow/BeginCol` ＝ `c1.row/col`）；
/// * `bottom/right` 取**右端格跨度右下角**（`row + rowMerge - 1` / `col + colMerge - 1`）；
/// * 命中判据 `cell.getRow() == i && cell.getCol() == j` —— `_$26` 里的合并延续槽
///   是**别名**（自身坐标指向锚点）⇒ 由此被跳过。
fn rect_cells(a: &Expr, b: &Expr, ctx: &mut dyn EvalCtx) -> Result<Vec<CellId>> {
    let c1 = endpoint_cell(a, ctx)?;
    let c2 = endpoint_cell(b, ctx)?;
    let (top, left) = ctx.cell_pos(c1);
    let (r2, k2) = ctx.cell_pos(c2);
    let bottom = r2 + ctx.row_merge(c2) - 1;
    let right = k2 + ctx.col_merge(c2) - 1;
    let mut out = Vec::new();
    for i in top..=bottom {
        for j in left..=right {
            let Some(cell) = ctx.cell_at(i, j) else {
                continue;
            };
            if ctx.cell_pos(cell) != (i, j) {
                continue; // 合并延续槽（别名）：自身坐标不等于遍历坐标
            }
            ctx.test_value_gate(cell)?; // `cell.testValue()`（`Link.java:85`）
            out.push(cell);
        }
    }
    Ok(out)
}

/// 矩形端点必须是格引用，否则照产品抛错（`Link.java:65-66,75-76` 与
/// `ValueList.java:92-94,107-109` 都抛 `operator.cellLeftOperation` /
/// `cellRightOperation` —— 不静默降级成空集）。
fn endpoint_cell(expr: &Expr, ctx: &dyn EvalCtx) -> Result<CellId> {
    match expr {
        Expr::CellRef {
            id,
            absolute,
            provisional: _,
        } => resolve_cell(ctx, id, *absolute)
            .ok_or_else(|| EngineError::UnknownCell { id: id.clone() }),
        other => Err(EngineError::UnsupportedExpr {
            expr: format!("矩形端点不是格引用: {other:?}"),
        }),
    }
}
