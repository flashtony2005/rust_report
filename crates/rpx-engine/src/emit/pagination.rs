//! `pagination` 段的渲染 —— `CalcJson.renderPages(...)`（`tests/fixtures/CalcJson.java:140-192`）。
//!
//! # 四条必须照抄的细节
//!
//! 1. **键序**（`:177-185`）：`paper_w, paper_h, columns, page_count, x_pages, y_pages,
//!    row_num_per_page, pages`。对象在 [`super::json`] 里按插入序写，所以顺序即契约。
//! 2. **`paper_w` / `paper_h` 是 float 的 `toString`**（`JsonObj.put(String, float)`）：
//!    `595.0f` 写出来是 **`595.0`**（不是 `595`）—— 用
//!    [`super::num::java_float_to_string`]，不是 `json::int`。
//! 3. **每页键序**（`:165-175`）：`page, rows, [first_value], [last_value], col1_values`；
//!    两个可选键**只在首列非全空时**写（`first != null` / `last != null`）。
//! 4. **`first_value` / `last_value` 用 `String.valueOf`，`col1_values` 用 `value(v)`**
//!    （`:160-163`）—— 两个**不同**的渲染器：前者是 `Object.toString`（整数值的 `Double`
//!    保留 `.0`），后者与 `cells[].value` 同源（会把整数值的 `Double` 折叠成裸整数）。
//!    这正是 `super::val` 模块文档"两个函数，别混用"的第三处落点。
//!
//! # 网格未跑分页器时
//!
//! [`section`] 写裸 `null`。这不是"静默通过"：对拍器把 `null` 当作**没有该段**
//! （`tests/common/compare/sections.rs` 的 `opt_section`），于是夹具有段、引擎没有 ⇒
//! 报 `Missing`。产品在错误型夹具上写的也正是 `null`。

use crate::arena::CalcGrid;
use crate::pager::{PageInfo, Pagination};

use super::json;
use super::num;
use super::val;

/// `pagination` 段的内层对象；网格无分页结果时写 `null`。
pub fn section(grid: &CalcGrid) -> String {
    match &grid.pagination {
        None => "null".to_string(),
        Some(p) => render(p),
    }
}

/// 一个 [`Pagination`] 的 JSON 对象（键序见模块文档细节 1）。
fn render(p: &Pagination) -> String {
    json::obj(vec![
        ("paper_w", num::java_float_to_string(p.paper_w)),
        ("paper_h", num::java_float_to_string(p.paper_h)),
        ("columns", json::int(i64::from(p.columns))),
        ("page_count", json::int(i64::from(p.page_count))),
        ("x_pages", json::int(i64::from(p.x_pages))),
        ("y_pages", json::int(i64::from(p.y_pages))),
        ("row_num_per_page", json::int(i64::from(p.row_num_per_page))),
        ("pages", json::arr(p.pages.iter().map(page_json).collect())),
    ])
}

/// 一页（键序见模块文档细节 3）。
///
/// ⛔ `first_value` / `last_value` 由 `col1_values` 的**首末元素**导出，而不是另存一份：
/// 产品同样是在同一遍扫描里顺手记下（`CalcJson.java:159-162`），存两份迟早漂移。
fn page_json(p: &PageInfo) -> String {
    let mut f: Vec<(&'static str, String)> = vec![
        ("page", json::int(i64::from(p.page))),
        ("rows", json::int(i64::from(p.rows))),
    ];
    if let Some(first) = p.col1_values.first() {
        f.push(("first_value", json::quote(&val::java_tostring(first))));
    }
    if let Some(last) = p.col1_values.last() {
        f.push(("last_value", json::quote(&val::java_tostring(last))));
    }
    f.push((
        "col1_values",
        json::arr(p.col1_values.iter().map(val::render_value).collect()),
    ));
    json::obj(f)
}
