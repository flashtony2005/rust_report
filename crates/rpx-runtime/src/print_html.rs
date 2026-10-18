//! `workbook.json`（快照契约）→ 可直接打印的 HTML：一页一个 `<section>`。
//!
//! 为什么不另写一套分页/字体引擎：快照里已经有分页后的每一页、每行列尺寸（pt）、
//! 每格样式与合并区，缺的只是"把它们摆到纸面上"。HTML + `@page` 就能表达，
//! 再交给浏览器（见 [`crate::pdf`]）落成 PDF。**渲染口径与 Web 打印预览同源**：
//! 同一份 workbook.json，同一套 mm/pt 换算。
//!
//! 尺寸单位约定（踩过坑，写死在这）：纸张与页边距是 **mm**（`printSize`），
//! `rowData.h` / `columnData.w` 是 **px**（96dpi，`snapshot/geom.rs` 的 `px()`），
//! 而字号 `s.fs` 是 **pt**。三者不能混。首版把行列当 pt 处理，整页被多缩了
//! 4/3 倍——数字全挤成两行。可用宽高 = (纸宽 - 左右边距) × 96/25.4。
//!
//! 图表（`rpxGraphics` 里 `kind=chart`）**不渲染**：CLI 没有图表光栅化能力。
//! 会画一个虚线占位框把标题写出来，同时由调用方记 `EXPORT_PARTIAL` 诊断——
//! 让"这里少了个图"是**显式**的，而不是留一片空白。

use serde_json::Value as J;

mod graphics;

/// 渲染结果：HTML + 页数 + 没能渲染的图表标题（用于诊断）。
#[derive(Debug)]
pub struct Rendered {
    pub html: String,
    pub pages: usize,
    pub charts: Vec<String>,
}

/// 一份工作的 workbook → 完整 HTML 文档。
pub fn from_workbook(workbook: &J) -> Result<Rendered, String> {
    let order = workbook
        .get("sheetOrder")
        .and_then(J::as_array)
        .ok_or("workbook 缺少 sheetOrder")?;
    let sheets = workbook.get("sheets").ok_or("workbook 缺少 sheets")?;
    let mut body = String::new();
    let mut charts = Vec::new();
    let mut pages = 0usize;
    let mut paper = (210.0, 297.0);
    for (index, id) in order.iter().enumerate() {
        let id = id.as_str().ok_or("sheetOrder 里有非字符串项")?;
        let sheet = sheets.get(id).ok_or_else(|| format!("缺少页 {id}"))?;
        let page = page_html(sheet, &mut charts)?;
        if index == 0 {
            paper = paper_of(sheet);
        }
        body.push_str(&page);
        pages += 1;
    }
    if pages == 0 {
        return Err("workbook 里没有任何页".into());
    }
    // 同一个图表在多页重复出现时只报一次——诊断是给人看的清单，不是出现次数统计。
    let mut seen = Vec::new();
    for title in charts {
        if !seen.contains(&title) {
            seen.push(title);
        }
    }
    let title = workbook
        .get("name")
        .and_then(J::as_str)
        .unwrap_or("rpx report");
    Ok(Rendered {
        html: document(title, &body, paper),
        pages,
        charts: seen,
    })
}

fn paper_of(sheet: &J) -> (f64, f64) {
    let size = &sheet["printSize"];
    (number(&size["width"]), number(&size["height"]))
}

fn page_html(sheet: &J, charts: &mut Vec<String>) -> Result<String, String> {
    let (paper_w, paper_h) = paper_of(sheet);
    let margins = margins_of(sheet);
    let rows = count_of(&sheet["printRowCount"]);
    let cols = count_of(&sheet["printColumnCount"]);
    let heights = axis(
        &sheet["rowData"],
        rows,
        "h",
        number(&sheet["defaultRowHeight"]),
    );
    let widths = axis(
        &sheet["columnData"],
        cols,
        "w",
        number(&sheet["defaultColumnWidth"]),
    );
    let mut content_w: f64 = widths.iter().sum();
    let mut content_h: f64 = heights.iter().sum();
    // 图形可能比列更宽/更高（看板把多个列当画布用），算缩放比时要把它们算进来，
    // 否则图会被裁掉一块——Web 打印预览也是这么算的。
    if let Some(items) = sheet["rpxGraphics"].as_array() {
        for item in items {
            let col = count_of(&item["col"]);
            let row = count_of(&item["row"]);
            if col < widths.len() {
                content_w =
                    content_w.max(widths[..col].iter().sum::<f64>() + number(&item["width"]));
            }
            if row < heights.len() {
                content_h =
                    content_h.max(heights[..row].iter().sum::<f64>() + number(&item["height"]));
            }
        }
    }
    let available_w = (paper_w - margins[1] - margins[3]).max(1.0) * 96.0 / 25.4;
    let available_h = (paper_h - margins[0] - margins[2]).max(1.0) * 96.0 / 25.4;
    let scale = (available_w / content_w.max(1.0))
        .min(available_h / content_h.max(1.0))
        .min(1.0);
    let options = Options::of(sheet);
    let merges = merges_of(sheet);
    let backgrounds = graphics::cell_backgrounds(sheet);
    let mut html = String::new();
    html.push_str(&format!(
        "<section class=\"page\" aria-label=\"{}\" style=\"width:{paper_w}mm;height:{paper_h}mm;padding:{}mm {}mm {}mm {}mm\">",
        escape(sheet["name"].as_str().unwrap_or("page")),
        margins[0],
        margins[1],
        margins[2],
        margins[3]
    ));
    html.push_str(&format!(
        "<div class=\"canvas\" style=\"transform:scale({scale:.4})\">"
    ));
    // 必须给 table 显式宽度：`table-layout:fixed` 在 `width:auto` 下会按内容撑列，
    // 列宽形同虚设，数字直接压到相邻列（实测 75.59px 的列被撑到 96px）。
    html.push_str(&format!(
        "<table style=\"width:{content_w:.2}px\"><colgroup>"
    ));
    for width in &widths {
        html.push_str(&format!("<col style=\"width:{width:.2}px\">"));
    }
    html.push_str("</colgroup><tbody>");
    for (row, height) in heights.iter().enumerate().take(rows) {
        html.push_str(&format!("<tr style=\"height:{height:.2}px\">"));
        for col in 0..cols {
            let (row_span, col_span, anchor) = span_at(&merges, row, col);
            if !anchor {
                continue;
            }
            let cell = sheet["cellData"]
                .get(row.to_string())
                .and_then(|r| r.get(col.to_string()));
            let background = backgrounds.get(&(row, col));
            html.push_str(&td_html(cell, row_span, col_span, &options, background));
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table>");
    html.push_str(&graphics::html(sheet, &widths, &heights, charts));
    html.push_str("</div></section>");
    Ok(html)
}

/// 一格的 `<td>`：样式按 Univer 的编码还原（`ht`/`vt`/`fs`pt/`cl`/`bg`/`n.pattern`）。
fn td_html(
    cell: Option<&J>,
    row_span: usize,
    col_span: usize,
    options: &Options,
    background: Option<&String>,
) -> String {
    let mut style = String::new();
    if let Some(cell) = cell {
        let s = &cell["s"];
        let font = s["ff"].as_str().unwrap_or("Microsoft YaHei");
        // 字体名用**单引号**：整个 style 是双引号属性，里面再写双引号会把属性截断，
        // 后半段样式静默失效（首版就是这样丢掉了对齐与 nowrap）。
        style.push_str(&format!(
            "font-family:'{}','PingFang SC',sans-serif;",
            font.replace('\'', "")
        ));
        if let Some(size) = s["fs"].as_f64() {
            style.push_str(&format!("font-size:{size:.2}pt;"));
        }
        if truthy(&s["bl"]) {
            style.push_str("font-weight:700;");
        }
        if truthy(&s["it"]) {
            style.push_str("font-style:italic;");
        }
        if s["ul"].is_object() {
            style.push_str("text-decoration:underline;");
        }
        if let Some(rgb) = s["cl"]["rgb"].as_str() {
            style.push_str(&format!("color:{rgb};"));
        }
        if let Some(rgb) = s["bg"]["rgb"].as_str() {
            style.push_str(&format!("background-color:{rgb};"));
        }
        style.push_str(&format!("text-align:{};", horizontal(&s["ht"])));
        style.push_str(&format!("vertical-align:{};", vertical(&s["vt"])));
    }
    if options.border_width > 0.0 {
        style.push_str(&format!(
            "border:{:.2}pt solid {};",
            options.border_width, options.border_color
        ));
    }
    if options.wrap_numbers {
        style.push_str("overflow-wrap:anywhere;");
    } else if cell.is_some_and(|c| c["t"].as_u64() == Some(2)) {
        // 数字默认不折行（Excel 的"数字不换行"），超宽就裁掉。
        style.push_str("white-space:pre;");
    }
    if let Some(url) = background {
        style.push_str(&format!(
            "background-image:url({});background-size:100% 100%;",
            escape(url)
        ));
    }
    let mut attrs = String::new();
    if row_span > 1 {
        attrs.push_str(&format!(" rowspan=\"{row_span}\""));
    }
    if col_span > 1 {
        attrs.push_str(&format!(" colspan=\"{col_span}\""));
    }
    // 内容包一层 `<div>`：`td` 上的 `overflow:hidden` 在 Chrome 里不生效（表格格
    // 不裁内容），数字会横着压到相邻列。块级子元素才真的裁。
    format!(
        "<td{attrs} style=\"{style}\"><div class=\"cell\">{}</div></td>",
        cell_text(cell)
    )
}

fn cell_text(cell: Option<&J>) -> String {
    let Some(cell) = cell else {
        return String::new();
    };
    let value = &cell["v"];
    if value.is_null() {
        return String::new();
    }
    let numeric = cell["t"].as_u64() == Some(2);
    if numeric {
        if let Some(number) = value.as_f64() {
            let pattern = cell["s"]["n"]["pattern"].as_str();
            let text = match pattern {
                Some(pattern) => crate::numfmt::format(pattern, number),
                None => crate::numfmt::format("0.##########", number),
            };
            if let Some(text) = text {
                return escape(&text);
            }
            return escape(&number.to_string());
        }
    }
    match value {
        J::String(text) => escape(text),
        J::Bool(flag) => flag.to_string(),
        other => escape(&other.to_string()),
    }
}

struct Options {
    border_width: f64,
    border_color: String,
    wrap_numbers: bool,
}

impl Options {
    fn of(sheet: &J) -> Self {
        let options = &sheet["printOptions"];
        Self {
            border_width: number(&options["borderWidth"]),
            border_color: options["borderColor"]
                .as_str()
                .unwrap_or("#000000")
                .to_string(),
            wrap_numbers: truthy(&options["wrapNumbers"]),
        }
    }
}

/// 合并区：`(起始行, 起始列, 结束行, 结束列)`，0 起（快照口径）。
fn merges_of(sheet: &J) -> Vec<(usize, usize, usize, usize)> {
    sheet["mergeData"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|m| {
                    (
                        count_of(&m["startRow"]),
                        count_of(&m["startColumn"]),
                        count_of(&m["endRow"]),
                        count_of(&m["endColumn"]),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 命中的合并区 → `(行跨度, 列跨度, 是否锚点)`；锚点才输出 `<td>`。
fn span_at(
    merges: &[(usize, usize, usize, usize)],
    row: usize,
    col: usize,
) -> (usize, usize, bool) {
    for (r0, c0, r1, c1) in merges {
        if row >= *r0 && row <= *r1 && col >= *c0 && col <= *c1 {
            return (r1 - r0 + 1, c1 - c0 + 1, row == *r0 && col == *c0);
        }
    }
    (1, 1, true)
}

fn margins_of(sheet: &J) -> [f64; 4] {
    let mut out = [0.0; 4];
    if let Some(items) = sheet["printSize"]["margins"].as_array() {
        for (i, value) in items.iter().take(4).enumerate() {
            out[i] = number(value);
        }
    }
    out
}

/// 行/列尺寸序列：缺项取默认值，`hd` 为真的项按 0 处理（隐藏不占位）。
fn axis(data: &J, count: usize, key: &str, default: f64) -> Vec<f64> {
    (0..count)
        .map(|i| {
            let Some(item) = data.get(i.to_string()) else {
                return default;
            };
            if truthy(&item["hd"]) {
                0.0
            } else {
                item[key].as_f64().unwrap_or(default)
            }
        })
        .collect()
}

fn document(title: &str, body: &str, paper: (f64, f64)) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"zh-CN\"><head><meta charset=\"utf-8\">\
<title>{}</title><style>\
@page{{size:{:.2}mm {:.2}mm;margin:0}}\
html,body{{margin:0;padding:0;background:#fff}}\
.page{{position:relative;box-sizing:border-box;overflow:hidden;break-after:page;page-break-after:always}}\
.page:last-child{{break-after:auto;page-break-after:auto}}\
.canvas{{transform-origin:top left}}\
table{{border-collapse:collapse;table-layout:fixed;margin:0}}\
td{{padding:0;white-space:pre-wrap;line-height:1.25;word-break:break-word}}\n.cell{{overflow:hidden}}\
.chart-missing{{position:absolute;box-sizing:border-box;display:flex;align-items:center;justify-content:center;\
border:1px dashed #b0b7c0;background:#fafbfc;color:#666;font:10pt sans-serif;text-align:center;padding:4px}}\
</style></head><body>{}</body></html>",
        escape(title),
        paper.0,
        paper.1,
        body
    )
}

fn horizontal(code: &J) -> &'static str {
    match code.as_i64() {
        Some(2) => "center",
        Some(3) => "right",
        Some(4) => "justify",
        _ => "left",
    }
}

fn vertical(code: &J) -> &'static str {
    match code.as_i64() {
        Some(1) => "top",
        Some(3) => "bottom",
        _ => "middle",
    }
}

fn number(value: &J) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

fn count_of(value: &J) -> usize {
    value.as_u64().unwrap_or(0) as usize
}

fn truthy(value: &J) -> bool {
    match value {
        J::Bool(flag) => *flag,
        J::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        _ => false,
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
