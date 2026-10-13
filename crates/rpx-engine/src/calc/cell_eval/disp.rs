//! `calcDispValue`（`ExtNormalCell.java:678-700`）的**可达子集**。
//!
//! # 产品原文的两段式
//!
//! ```text
//! format = getFormat()
//! if propEngMap != null:
//!     键 49（格式表达式）→ 求值成 String 则 setFormat        :683-688
//!     键 41（显示值表达式）→ 求值 → format 空则 toString     :689-695
//!                           否则 Variant2.format → 直接返回
//! dispValue = engine.srcPrepare.getDispValue()               :696（模板声明）
//! format 非空：dispValue==null && cellValue!=null
//!              ? format(cellValue) : format(parse(dispValue)) :697-699
//! format 空：返回 dispValue（模板声明）                       :699-700
//! ```
//!
//! # 本 crate 的建模边界（诚实声明）
//!
//! - `propEngMap`（键 41/49 的**显示值/格式表达式**）未建模 —— 12 夹具的
//!   dispValue 全部来自模板声明（`engine_disp` 的"仅显示值"即
//!   `srcPrepare.getDispValue()`），无夹具走键 41 分支；
//! - `getFormat()` / `Variant2.format` 未建模（M2-14 已声明缺口）⇒ `format`
//!   视作恒空，走 `:699-700` 的"直接返回声明"分支。
//!
//! 若将来夹具出现 format 声明，这里必须先回源码补 `Variant2.format`，不能静默
//! 返回声明。
//!
//! # ⛔ 读的是**模板声明**，不是 getter 的合成值
//!
//! `:696` 是 `engine.srcPrepare.getDispValue()` —— 只读模板那一支。而
//! [`crate::heads::HeadCell::disp_value`] 是"覆盖优先"的 getter。扩展格**第一趟**
//! 每个元素都调 `calcDispValue`，若误读 getter 会把本格上次 `setDispValue` 写入的
//! 覆盖值读回来（Java 的 `propSetMap[41]` 与 `srcPrepare` 是两个存储，`:696` 只碰
//! 后者）。所以这里直读 `disp_declared`。

use crate::arena::CellId;
use crate::calc::ctx::CalcCtx;
use crate::error::EngineError;

/// `calcDispValue(ctx)`：返回 `Option<String>`（Java 的 null ⇒ `None`）。
///
/// 参数取 `&CalcCtx` 即可：建模边界内没有任何求值（将来补键 41/49 表达式时
/// 才需要 `&mut` —— 且必须在元素 Row 态指针下求值，调用点顺序已按 Java 排好）。
pub(crate) fn calc_disp_value(ctx: &CalcCtx, id: CellId) -> Result<Option<String>, EngineError> {
    Ok(ctx.grid.get(id).and_then(|h| h.disp_declared.clone()))
}
