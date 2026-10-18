//! 列宽：模板列宽（px）+ 「数字必须放得下」的下限。
//!
//! # 为什么存在
//!
//! Excel 对**数字**放不下时的处理不是截断、也不是溢出到邻格，而是把整格画成
//! `####`。模板列宽是按设计稿定的固定值，而真实数据比设计稿宽——大额金额、
//! 两位小数、占比的长尾小数——于是导出的 xlsx 一打开就是一片 `####`。
//! 这不是数字格式问题，是**列宽**问题：同一个值只要列宽够，Excel 就能正常显示。
//!
//! 首版更糟：**数据出口 `to_xlsx` 根本没写 `<cols>`**，列宽全是 Excel 默认的
//! 8.43 字符（≈64px），连 `1286000` 这种七位数都悬。
//!
//! # 口径（三条，都是有意的）
//!
//! 1. **只对数值格加宽**。文本放不下时 Excel 会溢出到邻格（相邻格为空）或截断，
//!    不会画 `####`；按文本加宽会破坏用户排好的版式，收益为负。
//! 2. **合并区按整段宽度算**。锚点格的显示宽度是跨度内各列之和，缺口**平均**
//!    摊到跨度内的可见列——不重新分配比例，避免一列被撑、相邻列留白。
//! 3. **隐藏列（宽 0）不参与**，也不会被加宽。
//!
//! 估算不可能和 Excel 的字距完全一致：宁可宽一点（多几个像素的留白看得到），
//! 不可窄一点（窄了就退回 `####`，用户看到的是坏数据）。
//!
//! 单位：`rowData.h`/`columnData.w` 与 Excel 列宽最终都落 **px**（96dpi）；
//! 模板列宽的单位是 [`CalcGrid::unit`]（常见是 mm），故先经
//! [`unit_to_pt`] 转 pt 再转 px。

use std::collections::HashMap;

use rpx_engine::arena::{CalcCell, CalcGrid};
use rpx_engine::numfmt;
use rpx_engine::pager::unit_to_pt;
use rpx_engine::value::Value;

/// 没设字号时的估算基准（pt）。
const DEFAULT_FONT_PT: f64 = 11.0;
/// 单元格左右内边距 + 网格线（Excel 实测约 6px），再留 2px 余量。
const PADDING_PX: f64 = 8.0;
/// 格式引擎不认识 pattern（日期等）时的兜底：按 10 字符估（`2025-09-30`）。
const UNKNOWN_CHARS: f64 = 10.0;
/// Excel 列宽硬上限（字符数）。
const MAX_COL_CHARS: f64 = 255.0;
/// Excel 列宽单位 ≈ 7px（96dpi、默认字体 11pt）。
const COL_CHAR_PX: f64 = 7.0;
/// 没写列宽时的 Excel 默认列宽：8.43 字符（≈64px）。
const DEFAULT_COL_PX: f64 = 64.0;

/// 模板列宽 → px（隐藏列为 0）。索引 0 ↔ 网格第 1 列。
pub(crate) fn base_px(grid: &CalcGrid) -> Vec<f64> {
    (0..grid.cols.max(0) as usize)
        .map(|i| {
            if grid.col_visible.get(i) == Some(&false) {
                return 0.0;
            }
            let width = grid.col_widths.get(i).copied().unwrap_or(0.0);
            unit_to_pt(width, grid.unit) * 96.0 / 72.0
        })
        .collect()
}

/// 在 `base` 之上把「数字放不下」的列加宽到刚好放得下（只加不减）。
pub(crate) fn fitted(grid: &CalcGrid, base: &[f64]) -> Vec<f64> {
    let mut widths = base.to_vec();
    if widths.is_empty() {
        return widths;
    }
    let spans = span_map(grid);
    for cell in &grid.cells {
        let Some((text, bold)) = numeric_text(grid, cell) else {
            continue;
        };
        let span = span_of(&spans, cell);
        let start = cell.col.max(1) as usize - 1;
        let end = (start + span).min(widths.len());
        // 隐藏列（`col_visible = false`）不参与：Excel 里它的内容本来就看不见。
        let active: Vec<usize> = (start..end)
            .filter(|i| grid.col_visible.get(*i) != Some(&false))
            .collect();
        if active.is_empty() {
            continue;
        }
        let have: f64 = active.iter().map(|i| usable(widths[*i])).sum();
        let size = grid
            .print_styles
            .get(&cell.origin)
            .map_or(DEFAULT_FONT_PT, |s| f64::from(s.size));
        let need = measure(&text, size, bold) + PADDING_PX;
        if need <= have {
            continue;
        }
        // 缺口平均摊：跨度内的列一起变宽，相对关系不变。
        let share = (need - have) / active.len() as f64;
        for i in active {
            let cap = MAX_COL_CHARS * COL_CHAR_PX;
            widths[i] = (usable(widths[i]) + share).min(cap);
        }
    }
    widths
}

/// 一格列宽的**可用 px**：没写列宽（手搭网格 / 模板漏项）时按 Excel 默认 8.43 字符算，
/// 否则"未设宽"会被当成"宽 0"而永远判成放得下，`####` 依旧。
fn usable(width_px: f64) -> f64 {
    if width_px > 0.0 {
        width_px
    } else {
        DEFAULT_COL_PX
    }
}

/// 数值格的**显示文本** + 是否加粗；非数值格返回 `None`（不参与加宽）。
///
/// ⛔ 每个数值分支都要过 [`number_text`]：格式是**按格**声明的，与值的存储类型无关。
/// 首版只对 `Double`/`Float` 套格式，`Int` 的 `1286000` 就按裸数字估宽（7 字符），
/// 而 Excel 显示的是 `1,286,000.00`（12 字符）——列宽算够了、Excel 照样 `####`。
fn numeric_text(grid: &CalcGrid, cell: &CalcCell) -> Option<(String, bool)> {
    if !is_number(&cell.value) {
        return None;
    }
    let style = grid.print_styles.get(&cell.origin);
    let bold = style.is_some_and(|s| s.bold);
    if let Some(disp) = &cell.disp_value {
        // `disp_value` 是引擎给死的显示文本，导出同样用它。
        return Some((disp.clone(), bold));
    }
    let pattern = style.and_then(|s| s.format.as_deref());
    let (text, known) = match &cell.value {
        Value::Int(n) => number_text(pattern, f64::from(*n)),
        Value::Long(n) => number_text(pattern, *n as f64),
        Value::Short(n) => number_text(pattern, f64::from(*n)),
        Value::Byte(n) => number_text(pattern, f64::from(*n)),
        Value::Double(f) => number_text(pattern, *f),
        Value::Float(f) => number_text(pattern, f64::from(*f)),
        Value::Decimal(s) => match s.parse::<f64>() {
            Ok(value) => number_text(pattern, value),
            // 超精度 Decimal 落到 f64 会失真，按原样估宽。
            Err(_) => (s.clone(), pattern.is_none()),
        },
        _ => return None,
    };
    let mut text = text;
    if !known {
        // 认不出的格式（日期等）按固定字符数估——日期列按 General 估会偏窄。
        let pad = (UNKNOWN_CHARS as usize).saturating_sub(text.chars().count());
        text.push_str(&"0".repeat(pad));
    }
    Some((text, bold))
}

/// 值 + 格式 → Excel 会显示出来的那串字；(文本, 是否认得出格式)。
fn number_text(pattern: Option<&str>, value: f64) -> (String, bool) {
    match pattern {
        Some(pattern) => match numfmt::format(pattern, value) {
            Some(text) => (text, true),
            None => (general(value), false),
        },
        None => (general(value), true),
    }
}

fn is_number(value: &Value) -> bool {
    matches!(
        value,
        Value::Int(_)
            | Value::Long(_)
            | Value::Short(_)
            | Value::Byte(_)
            | Value::Decimal(_)
            | Value::Double(_)
            | Value::Float(_)
    )
}

/// Excel General 口径的**估宽文本**：整数部分 + 两位小数预算。
///
/// ⛔ 不是 Excel 真正显示的那串：General **会按列宽收减小数位**（0.3954489 在窄列
/// 显示成 `0.4`，在宽列显示成 `0.395448954`）——它只在**整数部分放不下**时才画
/// `####`。按全精度估宽会把这类列撑成两倍宽，纯属浪费版面。
fn general(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let integer = format!("{:.0}", value.trunc());
    if value.fract() == 0.0 {
        integer
    } else {
        format!("{integer}.00")
    }
}

/// 合并跨度表：`(行,列)` → 横向跨度（同区各槽都报同一个跨度）。
fn span_map(grid: &CalcGrid) -> HashMap<(i32, i32), i32> {
    let mut spans = HashMap::new();
    for item in &grid.merged {
        if item.col_merge > 1 {
            spans.insert((item.r, item.c), item.col_merge);
        }
    }
    spans
}

fn span_of(spans: &HashMap<(i32, i32), i32>, cell: &CalcCell) -> usize {
    spans
        .get(&(cell.row, cell.col))
        .map_or(1, |span| (*span).max(1) as usize)
}

/// 文本估宽（px）：字符按 em 系数累加，再乘字号。
fn measure(text: &str, size_pt: f64, bold: bool) -> f64 {
    let em = size_pt.max(1.0) * 96.0 / 72.0;
    let units: f64 = text.chars().map(advance).sum();
    let width = units * em;
    if bold {
        width * 1.05
    } else {
        width
    }
}

/// 单字符的 em 系数（Calibri/Arial 量级的近似）。
fn advance(ch: char) -> f64 {
    match ch {
        '0'..='9' => 0.55,
        '.' | ',' | ':' | ';' | ' ' | '/' => 0.28,
        '%' => 0.95,
        '-' => 0.36,
        '+' => 0.58,
        '¥' | '$' | '€' => 0.62,
        'A'..='Z' => 0.66,
        'a'..='z' => 0.52,
        _ if (ch as u32) >= 0x2E80 => 1.0,
        _ => 0.55,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpx_engine::pager::style::PrintStyle;

    fn style_of(pattern: &str) -> PrintStyle {
        PrintStyle {
            font: "Microsoft YaHei".into(),
            size: 12,
            bold: false,
            italic: false,
            underline: false,
            foreground: 0,
            background: 0xFFFFFF,
            horizontal: -47,
            vertical: 0,
            format: Some(pattern.to_string()),
            stretch_when_paged: false,
            split: false,
            borders: [(0, 0.0, 0); 4],
        }
    }

    fn grid(width_mm: f32, value: Value, format: Option<&str>) -> CalcGrid {
        let mut grid = CalcGrid::new(1, 1);
        grid.unit = 1;
        grid.col_widths = vec![width_mm];
        grid.col_visible = vec![true];
        grid.cells = vec![CalcCell::bare(1, 1, value)];
        if let Some(pattern) = format {
            grid.print_styles.insert((1, 1), style_of(pattern));
        }
        grid
    }

    /// 20mm 的列放 `#,##0.00` 的 1286000 —— 设计稿宽度放不下，必须被加宽。
    #[test]
    fn narrow_column_grows_to_fit_money() {
        let grid = grid(20.0, Value::Double(1_286_000.0), Some("#,##0.00"));
        let base = base_px(&grid);
        assert!(
            (base[0] - 75.59).abs() < 0.5,
            "20mm ≈ 75.59px，实得 {}",
            base[0]
        );
        let fitted = fitted(&grid, &base);
        assert!(
            fitted[0] > base[0],
            "放不下时必须加宽：{} -> {}",
            base[0],
            fitted[0]
        );
        let need = measure("1,286,000.00", 12.0, false) + PADDING_PX;
        assert!(
            fitted[0] >= need,
            "加宽后仍放不下：{} < {}",
            fitted[0],
            need
        );
    }

    /// 格式按**格**声明，与值的存储类型无关：`int` 的 1286000 在 Excel 里显示成
    /// `1,286,000.00`，估宽必须按格式化后的文本算。
    #[test]
    fn integer_with_money_format_is_measured_as_money() {
        let grid = grid(20.0, Value::Int(1_286_000), Some("#,##0.00"));
        let base = base_px(&grid);
        assert!(
            fitted(&grid, &base)[0] > base[0],
            "int + 金额格式没被加宽：{}",
            base[0]
        );
    }

    /// 宽列不动 —— 「只加不减」。
    #[test]
    fn wide_column_is_left_alone() {
        let grid = grid(60.0, Value::Double(1_286_000.0), Some("#,##0.00"));
        let base = base_px(&grid);
        assert_eq!(fitted(&grid, &base), base);
    }

    /// 没写列宽（手搭网格 / 模板漏项）按 Excel 默认 8.43 字符（≈64px）兜底，
    /// 否则"未设宽"会被当成"宽 0"，永远判成放得下。
    #[test]
    fn unset_width_falls_back_to_excel_default() {
        let mut grid = CalcGrid::new(1, 1);
        grid.unit = 1;
        grid.col_visible = vec![true];
        grid.cells = vec![CalcCell::bare(1, 1, Value::Double(1_286_000.0))];
        grid.print_styles.insert((1, 1), style_of("#,##0.00"));
        let base = base_px(&grid);
        assert_eq!(base, vec![0.0]);
        assert!(fitted(&grid, &base)[0] > DEFAULT_COL_PX);
    }

    /// 隐藏列不参与，也不会被加宽。
    #[test]
    fn hidden_column_is_left_alone() {
        let mut grid = grid(20.0, Value::Double(1_286_000.0), Some("#,##0.00"));
        grid.col_visible = vec![false];
        let base = base_px(&grid);
        assert_eq!(fitted(&grid, &base), base);
    }

    /// 文本不放宽（Excel 文本溢出到邻格，不会变 `####`）。
    #[test]
    fn text_cells_do_not_widen() {
        let grid = grid(20.0, Value::Text("华东机械集团有限责任公司".into()), None);
        let base = base_px(&grid);
        assert_eq!(fitted(&grid, &base), base);
    }

    /// 无格式的数按 Excel General 估：**只有整数部分**必须放得下，小数位由 Excel
    /// 按列宽自适应。按全精度估（`0.39544895448954487`）会把列撑成两倍宽。
    #[test]
    fn general_only_reserves_the_integer_part() {
        assert_eq!(general(1_286_000.0), "1286000");
        assert_eq!(general(0.395_448_954_489_544_87), "0.00");
        assert_eq!(general(-42.5), "-42.00");
    }
}
