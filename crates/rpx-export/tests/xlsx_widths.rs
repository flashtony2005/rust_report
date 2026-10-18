//! 回归：导出的 xlsx **必须带列宽**，且数字放得下。
//!
//! 缺陷现场：`to_xlsx` 首版不写 `<cols>`，Excel 按默认 8.43 字符（≈64px）显示，
//! `#,##0.00` 的金额、`0.0%` 的占比一律被画成 `####`。用户看到的是"坏数据"。
//! 这里直接读回 `xl/worksheets/sheet1.xml` 的 `<cols>` 断言，不靠肉眼。

use std::io::{Cursor, Read};

use rpx_engine::arena::{CalcCell, CalcGrid};
use rpx_engine::pager::style::PrintStyle;
use rpx_engine::value::Value;

/// 2 列：第 1 列 20mm 放一笔 `#,##0.00` 的金额（设计稿宽度放不下），
/// 第 2 列 60mm 放同样的值（放得下，不该被动）。
fn grid() -> CalcGrid {
    let mut grid = CalcGrid::new(1, 2);
    grid.unit = 1; // mm
    grid.col_widths = vec![20.0, 60.0];
    grid.col_visible = vec![true, true];
    grid.cells = vec![
        CalcCell::bare(1, 1, Value::Double(1_286_000.0)),
        CalcCell::bare(1, 2, Value::Double(1_286_000.0)),
    ];
    grid.print_styles.insert((1, 1), style("#,##0.00"));
    grid.print_styles.insert((1, 2), style("#,##0.00"));
    grid
}

fn style(pattern: &str) -> PrintStyle {
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

/// 读回第一张工作表的 XML。
fn sheet_xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("xlsx 是合法 zip");
    let mut entry = archive
        .by_name("xl/worksheets/sheet1.xml")
        .expect("有 sheet1.xml");
    let mut xml = String::new();
    entry.read_to_string(&mut xml).expect("xml 是 utf-8");
    xml
}

/// `<col min="N" ... width="W"/>` → `(N, W)`。
fn cols(xml: &str) -> Vec<(u32, f64)> {
    let mut out = Vec::new();
    for piece in xml.split("<col ").skip(1) {
        let piece = piece.split("/>").next().unwrap_or_default();
        let (mut min, mut width) = (None, None);
        for attr in piece.split_whitespace() {
            let Some((name, value)) = attr.split_once("=\"") else {
                continue;
            };
            let value = value.trim_end_matches('"');
            match name {
                "min" => min = value.parse().ok(),
                "width" => width = value.parse().ok(),
                _ => {}
            }
        }
        if let (Some(min), Some(width)) = (min, width) {
            out.push((min, width));
        }
    }
    out
}

/// 数据出口：以前完全没有 `<cols>`。
#[test]
fn plain_xlsx_writes_column_widths() {
    let xml = sheet_xml(&rpx_export::to_xlsx(&grid()));
    let widths = cols(&xml);
    assert_eq!(widths.len(), 2, "两列都要落列宽：{xml}");
    assert!(widths[0].1 > 0.0 && widths[1].1 > 0.0);
}

/// 窄列被加宽到放得下金额，宽列保持模板值。
#[test]
fn narrow_column_is_widened_wide_column_is_not() {
    let xml = sheet_xml(&rpx_export::to_xlsx(&grid()));
    let widths = cols(&xml);
    let narrow = widths[0].1;
    let wide = widths[1].1;
    // 20mm ≈ 75.59px ≈ 10.8 字符；`1,286,000.00` 12 个字符在 12pt 下放不下。
    assert!(narrow > 10.8, "第 1 列应被加宽，实得 {narrow}");
    // 60mm ≈ 226.77px ≈ 32.4 字符，早已放下 ⇒ 不被改动。
    assert!((wide - 32.4).abs() < 0.5, "第 2 列不该被改动，实得 {wide}");
}

/// 打印出口同样带列宽（此前只走模板宽度，真实数据会 `####`）。
#[test]
fn print_xlsx_writes_column_widths() {
    let grid = grid();
    let bytes = rpx_export::to_print_xlsx(&grid, &grid.print_setup).expect("打印导出成功");
    let xml = sheet_xml(&bytes);
    let widths = cols(&xml);
    assert_eq!(widths.len(), 2, "打印出口没写列宽：{xml}");
    assert!(widths[0].1 > 10.8, "窄列未按内容加宽：{}", widths[0].1);
}
