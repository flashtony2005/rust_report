//! 分页的**算术层**（2026-10-14 拆自 `pager.rs`，G4 行数门禁）。
//!
//! 三条算术是 `pager/tests.rs` 的首要测试对象（它们直接决定"算错页数"）：
//! 纸张 pt↔mm 往返的 float32/double 复刻（[`paper_size_pt`]）、
//! 单位换算不取整（[`unit_to_pt`]）、千分位容差（[`breaks`] / [`fits`]）。

/// 千分位容差常数（`SimplePager._$8`）。
const TOLERANCE: f64 = 1000.0;

/// 纸张 pt → mm → pt：复刻 `setPaperSize`（float32）与 `getPageHeight`（double）。
///
/// `(float)width * 25.4f / 72.0f` 是 **float32 左结合**运算；`mmToPixel` 在 double 上做。
/// `isHighPrecision == true` ⇒ **不取整**。842pt 的往返结果是 `841.9999730868603`。
pub fn paper_size_pt(pt: f32) -> f64 {
    let mm = pt * 25.4_f32 / 72.0_f32;
    f64::from(mm) * 72.0 / 25.4
}

/// `Pager.unitTransfer(f, unit, scale = 1.0f, isHighPrecision = true)`（`Pager.java:160-172`）。
///
/// * `unit == 2`（英寸）⇒ `* 72.0`；
/// * `unit == 1`（毫米，报表默认）⇒ `* 72.0 / 25.4`；
/// * 其余（`0` 等）⇒ 原值；
/// * `isHighPrecision` 为真 ⇒ **不 `Math.ceil`**。
pub fn unit_to_pt(v: f32, unit: i8) -> f64 {
    let d = f64::from(v);
    match unit {
        2 => d * 72.0,
        1 => d * 72.0 / 25.4,
        _ => d,
    }
}

/// Java 的**断页条件**（`SimplePager._$1:1513`、`_$2:2363` 的同一个式子）：
/// `cost > ceil((limit - used) * 1000) / 1000` 才放不下。
///
/// 与 [`fits`] 是同一式的两面。**故意拆成两个函数**：`>`（Java 原文）与取反分开写，
/// 免得读者把 `!(a > b)` 读成"方向变了"（clippy 的 `neg_cmp_op_on_partial_ord`
/// 也正是这条顾虑）。
pub(super) fn breaks(cost: f64, used: f64, limit: f64) -> bool {
    cost > ((limit - used) * TOLERANCE).ceil() / TOLERANCE
}

/// 这一页/这一栏**还放得下** ⇒ `!breaks(...)`。
pub(super) fn fits(cost: f64, used: f64, limit: f64) -> bool {
    !breaks(cost, used, limit)
}
