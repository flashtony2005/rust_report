//! 打印 HTML 与数字格式：把"肉眼才看得出"的两类错误钉成断言。
//!
//! 这两件事的共同点：**错了不会报错**。`#,##0.00` 不格式化只是少了千分位；
//! 列宽单位用错只是整页缩了 4/3 倍。所以必须有测试盯着具体字符串。
use rpx_runtime::{numfmt, print_html};
use serde_json::{json, Value};

#[test]
fn formats_the_six_patterns_the_examples_actually_use() {
    assert_eq!(
        numfmt::format("#,##0.00", 1_286_000.0).unwrap(),
        "1,286,000.00"
    );
    assert_eq!(numfmt::format("#,##0", 1_286_000.0).unwrap(), "1,286,000");
    assert_eq!(numfmt::format("0", 1286000.0).unwrap(), "1286000");
    assert_eq!(numfmt::format("0.0000", 1.5).unwrap(), "1.5000");
    assert_eq!(numfmt::format("0.00%", 0.3952).unwrap(), "39.52%");
    assert_eq!(numfmt::format("0.0%", 0.3952).unwrap(), "39.5%");
}

#[test]
fn handles_negative_sections_and_unsupported_patterns() {
    // 单个分段：负号由格式化器补。
    assert_eq!(numfmt::format("#,##0.00", -1500.5).unwrap(), "-1,500.50");
    // 负分段：负号来自分段字面量（`[Red]` 这类修饰会被剥掉）。
    assert_eq!(
        numfmt::format("#,##0.00;[Red]-#,##0.00", -1500.5).unwrap(),
        "-1,500.50"
    );
    // 日期格式没有数字占位符 → 明确放弃，让调用方回退纯文本（不猜）。
    assert_eq!(numfmt::format("yyyy-mm-dd", 45_000.0), None);
    assert_eq!(numfmt::format("#,##0.00", f64::NAN), None);
}

fn sheet(name: &str) -> Value {
    json!({
        "name": name,
        "printSize": {"width": 210.0, "height": 297.0, "margins": [10.0, 10.0, 10.0, 10.0]},
        "printRowCount": 3,
        "printColumnCount": 2,
        "defaultRowHeight": 30.0,
        "defaultColumnWidth": 90.0,
        "rowData": {"0": {"h": 40.0}, "1": {"h": 30.0}, "2": {"h": 30.0}},
        "columnData": {"0": {"w": 120.0}, "1": {"w": 100.0}},
        "printOptions": {"borderWidth": 0.5, "borderColor": "#000000", "wrapNumbers": false},
        "cellData": {
            "0": {"0": {"v": "月度金额", "t": 4, "s": {"ff": "Arial", "fs": 16, "bl": 1, "ht": 2, "vt": 2}}},
            "1": {
                "0": {"v": "金额", "t": 4, "s": {"ff": "Arial", "fs": 12, "ht": 1, "vt": 2}},
                "1": {"v": 1286000.0, "t": 2, "s": {"ff": "Arial", "fs": 12, "ht": 3, "vt": 2, "n": {"pattern": "#,##0.00"}}}
            }
        },
        "mergeData": [{"startRow": 0, "startColumn": 0, "endRow": 0, "endColumn": 1}],
        "rpxGraphics": [{
            "row": 2, "col": 0, "width": 200.0, "height": 100.0,
            "content": {"kind": "chart", "title": "销售额", "type": "column"}
        }]
    })
}

fn workbook() -> Value {
    json!({
        "name": "示例",
        "sheetOrder": ["s1", "s2"],
        "sheets": {"s1": sheet("第 1 页"), "s2": sheet("第 2 页")}
    })
}

#[test]
fn one_section_per_page_and_numbers_are_formatted() {
    let rendered = print_html::from_workbook(&workbook()).unwrap();
    assert_eq!(rendered.pages, 2);
    assert_eq!(rendered.html.matches("<section class=\"page\"").count(), 2);
    assert!(
        rendered.html.contains("1,286,000.00"),
        "数字必须按 pattern 格式化，否则版式全错"
    );
    assert!(
        rendered.html.contains("@page{size:210.00mm 297.00mm"),
        "纸张尺寸要进 @page，否则打印机会按自己的纸分页"
    );
    // 行/列尺寸是 px，字号是 pt；混用的症状是整页缩放比例错、数字被挤成两行。
    assert!(rendered.html.contains("width:120.00px"));
    assert!(rendered.html.contains("font-size:16.00pt"));
    // 合并区锚点必须带 colspan，否则标题只占第一列。
    assert!(rendered.html.contains("colspan=\"2\""));
}

#[test]
fn charts_are_reported_not_silently_dropped() {
    let rendered = print_html::from_workbook(&workbook()).unwrap();
    assert_eq!(rendered.charts, vec!["销售额".to_string()]);
    assert!(rendered.html.contains("chart-missing"));
    assert!(rendered.html.contains("销售额"));
}

#[test]
fn missing_sheet_order_is_an_error_not_a_blank_page() {
    let error = print_html::from_workbook(&json!({"sheets": {}})).unwrap_err();
    assert!(error.contains("sheetOrder"), "{error}");
}

/// 边框：`s.bd` 的每一条边都要落成 CSS；**没声明的那几边仍要有网格线**。
///
/// 这条钉的是「逐边兜底」而不是整格 `border` 简写 —— 简写会把用户画的框整片覆盖掉，
/// 表现是「预览/PDF 里边框时有时无」，很难从截图看出来。
#[test]
fn cell_borders_win_over_grid_lines_per_side() {
    let mut book = workbook();
    book["sheets"]["s1"]["cellData"]["1"]["0"]["s"]["bd"] = json!({
        "t": { "s": 1, "cl": { "rgb": "#FF0000" } },   // THIN  → 0.75pt solid
        "b": { "s": 7, "cl": { "rgb": "#00FF00" } },   // DOUBLE→ 2.25pt double
    });
    let html = print_html::from_workbook(&book).unwrap().html;

    assert!(
        html.contains("border-top:0.75pt solid #FF0000"),
        "上边框应有显式声明：{html}"
    );
    assert!(
        html.contains("border-bottom:2.25pt double #00FF00"),
        "下边框应为双线"
    );
    // 左右没声明 ⇒ 仍由 `printOptions.borderWidth`(0.5pt) 兜底
    assert!(
        html.contains("border-left:0.50pt solid #000000"),
        "左边应回落到网格线"
    );
    assert!(
        html.contains("border-right:0.50pt solid #000000"),
        "右边应回落到网格线"
    );
}
