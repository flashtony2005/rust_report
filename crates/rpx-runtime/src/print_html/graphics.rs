//! 页面上的非单元格内容：图片、单元格底色、以及**渲染不了的图表**。
//!
//! 图表占位是刻意设计：CLI 不做图表光栅化，留一片空白会被当成版式 bug；
//! 画一个画出标题的虚线框，配合 `EXPORT_PARTIAL` 诊断，"少了什么"就是显式的。
use super::{count_of, escape, number};
use serde_json::Value as J;
use std::collections::BTreeMap;

pub(super) fn html(sheet: &J, widths: &[f64], heights: &[f64], charts: &mut Vec<String>) -> String {
    let Some(items) = sheet["rpxGraphics"].as_array() else {
        return String::new();
    };
    let mut out = String::new();
    for item in items {
        let row = count_of(&item["row"]);
        let col = count_of(&item["col"]);
        if row >= heights.len() || col >= widths.len() {
            continue;
        }
        if heights[row] == 0.0 || widths[col] == 0.0 {
            continue;
        }
        let left: f64 = widths[..col].iter().sum();
        let top: f64 = heights[..row].iter().sum();
        let width = number(&item["width"]);
        let height = number(&item["height"]);
        let content = &item["content"];
        match content["kind"].as_str().unwrap_or("") {
            "cell_background" => continue,
            "image" => {
                if let Some(source) = content["source"].as_str() {
                    out.push_str(&format!(
                        "<img src=\"{}\" style=\"position:absolute;left:{left:.2}px;top:{top:.2}px;width:{width:.2}px;height:{height:.2}px\">",
                        escape(source)
                    ));
                }
                continue;
            }
            _ => {}
        }
        let title = content["title"]
            .as_str()
            .filter(|title| !title.is_empty())
            .unwrap_or("图表")
            .to_string();
        charts.push(title.clone());
        out.push_str(&format!(
            "<div class=\"chart-missing\" style=\"left:{left:.2}px;top:{top:.2}px;width:{width:.2}px;height:{height:.2}px\">{}</div>",
            escape(&format!("{title}（图表需图形预览，PDF 暂不渲染）"))
        ));
    }
    out
}

/// 单元格底色图形：`(行, 列) -> 图片地址`，由 `<td>` 的 background-image 承接。
pub(super) fn cell_backgrounds(sheet: &J) -> BTreeMap<(usize, usize), String> {
    let mut out = BTreeMap::new();
    if let Some(items) = sheet["rpxGraphics"].as_array() {
        for item in items {
            let content = &item["content"];
            if content["kind"].as_str() == Some("cell_background") {
                if let Some(source) = content["source"].as_str() {
                    out.insert(
                        (count_of(&item["row"]), count_of(&item["col"])),
                        source.into(),
                    );
                }
            }
        }
    }
    out
}
