//! 纸张码表与**模板真值口径**的可用页尺寸（M3-06）。
//!
//! # 与探针口径的分工
//!
//! 分页算法（[`super::split_rows`] / [`super::split_cols`]）只有一份；差在**页尺寸**：
//!
//! | 口径 | 页尺寸来源 | 消费者 |
//! | --- | --- | --- |
//! | 探针（M2-11 对拍） | [`super::PAPER_W_PT`] 常量 595×842pt、零边距 | `calculate()` → 夹具 `pagination` 段 |
//! | 模板真值（M3-06） | 本模块：纸码表 + 模板边距 | `/render` 快照 `pagination` 块 |
//!
//! 探针路径之所以长那样，是产品探针自己覆盖的：
//! `ReportUtils2.getPagerInfo` 把纸张改成传入的 595×842（pt→mm，
//! `setPaperSize` 同时把纸码置 **256=自定义**，`:74`/`:118-121`）并**清零四边边距**
//! （`:75-82`）。模板真值路径 = `new PagerInfo(PrintSetup)` 不覆盖
//! （`PagerInfo.java:43-68`）⇒ 纸码表 + 模板边距原样进断页。
//!
//! # 算术（`PagerInfo.getPageFormat(true)`，`:269-278`）
//!
//! ```text
//! paper_mm = 纸码表(paper) 或自定义字段（表中无该码，switch 默认分支）
//! usable_pt = (paper_mm - 边距) * 72.0 / 25.4      // mmToPixel，`:1150-1152`
//! ```
//!
//! `isHighPrecision` 恒 true（`ReportDefine._$16 = true`，`:75`）⇒ double 运算、
//! **不取整**（`getPageWidth(true)` 分支，`:241-244`）。与探针路径的
//! [`super::arith::paper_size_pt`] 是两套算术：探针把 pt 转成 mm 存进 float32
//! 字段再转回（有 842→841.99997… 的舍入），本路径的值**本来就以 mm 为单位**
//! （模板槽 9/10 是 f32 mm）⇒ 不做那次往返。
//!
//! # orient 不换宽高（faithful port）
//!
//! `PagerInfo.getPageFormat` 只把 orient 写进 `PageFormat.setOrientation`，
//! **不**交换 imageable 宽高（对照 `PrintSetup.java:920-935` 那个会把边距
//! 轮换的方法——SimplePager 走的是前者）。故本模块同样不换。
//!
//! # 表的来历
//!
//! `scripts/gen_paper_table.py` 从 `PrintSetup.java` 的两个 switch 生成
//! （height `:437-676` / width `:677-916`），**禁手抄**；字面量保留 Java 的
//! double 展开形态（float 常量的精确值）。

use rpx_core::print_setup::PrintSetup;

/// `PrintSetup.getPaperWidth(int paper, float defaultWidth)`：纸码 → 宽（mm）。
/// 表中无该码 ⇒ 自定义字段（Java switch 的默认分支，`:914-916`）。
pub fn paper_width_mm(paper: i16, custom_width: f32) -> f64 {
    paper_width_mm_table(paper).unwrap_or(f64::from(custom_width))
}

/// `PrintSetup.getPaperHeight(int paper, float defaultHeight)`：纸码 → 高（mm）。
pub fn paper_height_mm(paper: i16, custom_height: f32) -> f64 {
    paper_height_mm_table(paper).unwrap_or(f64::from(custom_height))
}

/// 模板真值口径的**可用页尺寸**（pt）：`PagerInfo.getPageFormat(true)` 的
/// `getImageableWidth/Height`（高精度 double，不取整）。
///
/// 即 `(纸mm − 左右边距) × 72/25.4` 与 `(纸mm − 上下边距) × 72/25.4`。
pub fn usable_page_pt(ps: &PrintSetup) -> (f64, f64) {
    let w_mm = paper_width_mm(ps.paper, ps.paper_width);
    let h_mm = paper_height_mm(ps.paper, ps.paper_height);
    let usable_w = (w_mm - f64::from(ps.left_margin) - f64::from(ps.right_margin)) * 72.0 / 25.4;
    let usable_h = (h_mm - f64::from(ps.top_margin) - f64::from(ps.bottom_margin)) * 72.0 / 25.4;
    (usable_w, usable_h)
}

/// 纸码 → 高/宽（mm）查找表（`paper_height_mm_table` 的 Rust 形态）。
/// 无该码 ⇒ `None`（调用方兜底自定义字段）。
fn paper_height_mm_table(paper: i16) -> Option<f64> {
    match paper {
        1 => Some(279.4),
        2 => Some(279.4),
        3 => Some(431.79999999999995),
        4 => Some(431.79999999999995),
        5 => Some(355.59999999999997),
        6 => Some(215.89999999999998),
        7 => Some(266.7),
        8 => Some(420.0),
        9 => Some(297.0),
        10 => Some(297.0),
        11 => Some(210.0),
        12 => Some(364.0),
        13 => Some(257.0),
        14 => Some(330.2),
        15 => Some(275.0),
        16 => Some(355.59999999999997),
        17 => Some(431.79999999999995),
        18 => Some(279.4),
        19 => Some(225.42499999999998),
        20 => Some(241.29999999999998),
        21 => Some(263.525),
        22 => Some(279.4),
        23 => Some(292.09999999999997),
        24 => Some(558.8),
        25 => Some(863.5999999999999),
        26 => Some(1117.6),
        27 => Some(220.0),
        28 => Some(229.0),
        29 => Some(458.0),
        30 => Some(324.0),
        31 => Some(162.0),
        32 => Some(229.0),
        33 => Some(353.0),
        34 => Some(250.0),
        35 => Some(176.0),
        36 => Some(230.0),
        37 => Some(190.5),
        38 => Some(165.1),
        39 => Some(279.4),
        40 => Some(304.79999999999995),
        41 => Some(330.2),
        43 => Some(148.0),
        44 => Some(279.4),
        45 => Some(279.4),
        46 => Some(279.4),
        47 => Some(220.0),
        50 => Some(304.79999999999995),
        51 => Some(381.0),
        52 => Some(457.2),
        53 => Some(322.0),
        54 => Some(279.4),
        55 => Some(297.0),
        56 => Some(304.79999999999995),
        57 => Some(356.0),
        58 => Some(487.0),
        59 => Some(322.2625),
        60 => Some(330.0),
        61 => Some(210.0),
        62 => Some(257.0),
        63 => Some(445.0),
        64 => Some(235.0),
        65 => Some(276.0),
        66 => Some(594.0),
        67 => Some(420.0),
        68 => Some(445.0),
        69 => Some(148.0),
        70 => Some(148.0),
        75 => Some(215.89999999999998),
        76 => Some(297.0),
        77 => Some(210.0),
        78 => Some(148.0),
        79 => Some(257.0),
        80 => Some(182.0),
        81 => Some(100.0),
        82 => Some(200.0),
        83 => Some(105.0),
        88 => Some(182.0),
        89 => Some(128.0),
        90 => Some(279.4),
        9999 => Some(500.0),
        _ => None,
    }
}

/// 纸码 → 高/宽（mm）查找表（`paper_width_mm_table` 的 Rust 形态）。
/// 无该码 ⇒ `None`（调用方兜底自定义字段）。
fn paper_width_mm_table(paper: i16) -> Option<f64> {
    match paper {
        1 => Some(215.89999999999998),
        2 => Some(215.89999999999998),
        3 => Some(279.4),
        4 => Some(279.4),
        5 => Some(215.89999999999998),
        6 => Some(139.7),
        7 => Some(184.14999999999998),
        8 => Some(297.0),
        9 => Some(210.0),
        10 => Some(210.0),
        11 => Some(148.0),
        12 => Some(257.0),
        13 => Some(182.0),
        14 => Some(215.89999999999998),
        15 => Some(215.0),
        16 => Some(254.0),
        17 => Some(279.4),
        18 => Some(215.89999999999998),
        19 => Some(98.425),
        20 => Some(104.77499999999999),
        21 => Some(114.3),
        22 => Some(120.64999999999999),
        23 => Some(127.0),
        24 => Some(431.79999999999995),
        25 => Some(558.8),
        26 => Some(863.5999999999999),
        27 => Some(110.0),
        28 => Some(162.0),
        29 => Some(324.0),
        30 => Some(229.0),
        31 => Some(114.0),
        32 => Some(114.0),
        33 => Some(250.0),
        34 => Some(176.0),
        35 => Some(125.0),
        36 => Some(110.0),
        37 => Some(98.425),
        38 => Some(92.07499999999999),
        39 => Some(377.825),
        40 => Some(215.89999999999998),
        41 => Some(215.89999999999998),
        43 => Some(100.0),
        44 => Some(228.6),
        45 => Some(254.0),
        46 => Some(381.0),
        47 => Some(220.0),
        50 => Some(241.29999999999998),
        51 => Some(241.29999999999998),
        52 => Some(296.8625),
        53 => Some(235.0),
        54 => Some(215.89999999999998),
        55 => Some(210.0),
        56 => Some(241.29999999999998),
        57 => Some(227.0),
        58 => Some(305.0),
        59 => Some(215.89999999999998),
        60 => Some(210.0),
        61 => Some(148.0),
        62 => Some(182.0),
        63 => Some(322.0),
        64 => Some(174.0),
        65 => Some(201.0),
        66 => Some(420.0),
        67 => Some(297.0),
        68 => Some(322.0),
        69 => Some(200.0),
        70 => Some(105.0),
        75 => Some(279.4),
        76 => Some(420.0),
        77 => Some(297.0),
        78 => Some(210.0),
        79 => Some(364.0),
        80 => Some(257.0),
        81 => Some(148.0),
        82 => Some(148.0),
        83 => Some(148.0),
        88 => Some(128.0),
        89 => Some(182.0),
        90 => Some(304.79999999999995),
        9999 => Some(353.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 附件 D 默认 setup（paper=9 A4、边距 19/19/25/25）。
    fn default_setup() -> PrintSetup {
        PrintSetup::default_values()
    }

    #[test]
    fn paper_codes_hit_the_table() {
        // A4（9）= 210×297；Letter（1）= 215.9×279.4；A3（8）= 297×420；
        // B3（9999）= 353×500。值与 PrintSetup.java 两个 switch 逐字一致。
        assert_eq!(paper_width_mm(9, 0.0), 210.0);
        assert_eq!(paper_height_mm(9, 0.0), 297.0);
        assert_eq!(paper_width_mm(1, 0.0), 215.89999999999998);
        assert_eq!(paper_height_mm(1, 0.0), 279.4);
        assert_eq!(paper_width_mm(8, 0.0), 297.0);
        assert_eq!(paper_height_mm(8, 0.0), 420.0);
        assert_eq!(paper_width_mm(9999, 0.0), 353.0);
        assert_eq!(paper_height_mm(9999, 0.0), 500.0);
    }

    #[test]
    fn unknown_code_falls_back_to_custom_fields() {
        // 256（探针 setPaperSize 置的自定义码）与任何未列出的码都走自定义字段。
        // ⛔ 自定义字段是 f32：Java 的 float→double 提升保留 f32 精度
        // （209.9f32 = 209.89999389648438），断言必须按同一提升比。
        assert_eq!(paper_width_mm(256, 209.9), f64::from(209.9_f32));
        assert_eq!(paper_height_mm(256, 297.0), 297.0);
        assert_eq!(paper_width_mm(-1, 100.0), 100.0);
        // 自定义字段为 0 时不兜底成 0 以外的值——Java 原样返回 defaultWidth。
        assert_eq!(paper_width_mm(256, 0.0), 0.0);
    }

    #[test]
    fn usable_page_subtracts_margins_in_pt() {
        let ps = default_setup();
        let (w, h) = usable_page_pt(&ps);
        // A4 210×297 − 边距 19/19/25/25 ⇒ 172×247mm ⇒ ×72/25.4。
        let expect_w = 172.0 * 72.0 / 25.4;
        let expect_h = 247.0 * 72.0 / 25.4;
        assert!((w - expect_w).abs() < 1e-12, "w={w}");
        assert!((h - expect_h).abs() < 1e-12, "h={h}");
        // 与探针口径（595×842 零边距）显然不同——正是 M3-06 要的差别。
        assert!(w < 595.0 && h < 842.0);
    }

    #[test]
    fn custom_paper_uses_custom_fields_with_margins() {
        let mut ps = default_setup();
        ps.paper = 256; // 自定义
        ps.paper_width = 100.0;
        ps.paper_height = 150.0;
        ps.left_margin = 10.0;
        ps.right_margin = 10.0;
        ps.top_margin = 5.0;
        ps.bottom_margin = 5.0;
        let (w, h) = usable_page_pt(&ps);
        assert!((w - 80.0 * 72.0 / 25.4).abs() < 1e-12, "w={w}");
        assert!((h - 140.0 * 72.0 / 25.4).abs() < 1e-12, "h={h}");
    }
}
