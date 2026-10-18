//! 动态合并格的两条来源，**不要混为一谈**（2026-10-18 实测分清）：
//!
//! | 来源 | 谁合并 | 怎么开 |
//! |------|--------|--------|
//! | 纵向扩展的**引擎跨度** | `group()` 生成的分组格，天然占满它那一块 | 不用开，`extend_mode: vertical` 自带 |
//! | **`merge_same`** 同值合并 | 明细列里**相邻等值**的格（分组格之外） | 必须显式写 `"merge_same": "vertical"` |
//!
//! ⛔ 判据别写成「给分组格加 `merge_same` 看有没有合并」：那是**空转**，
//! 去掉照样合并（分组格的跨度来自扩展，不是合并趟）。真正吃 `merge_same` 的是
//! [`detail_merge_same_joins_adjacent_equal_cells`] —— 它自带反证：同一份模板摘掉
//! `merge_same` 后必须一格都不并。
//!
//! # 为什么判据看 `mergeData` 而不是 `result.json` 的 `cells`
//!
//! 两趟合并只改**计算矩阵**，跨度只在 `CalcGrid::merged` 里可见
//! （见 `crates/rpx-engine/src/arena/calc_grid.rs` 的 `MergedCell` 文档），
//! 而 `cells` 仍逐格落盘 —— 拿 `cells` 数「大区列有几格」会得到 6 格，
//! 看上去像「没合并」。真值在 `workbook.json` 的 `mergeData`。
use rpx_engine::params::ParamTable;
use rpx_runtime::{execution::render_request, form::RenderRequest};
use serde_json::Value as J;
use std::{collections::BTreeMap, path::Path};

const DATA: &str = "crosstab-merge-cells.csv";

fn request(template: &str) -> RenderRequest {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/r-model");
    RenderRequest {
        report: std::fs::read(dir.join(format!("{template}.json"))).unwrap(),
        report_name: template.into(),
        params: ParamTable::EMPTY,
        datasets: BTreeMap::from([(DATA.to_string(), std::fs::read(dir.join(DATA)).unwrap())]),
    }
}

fn render(req: &RenderRequest) -> J {
    serde_json::from_str(&render_request(req).unwrap()).unwrap()
}

/// 某一列上所有纵向合并的跨度（行数），升序。
///
/// 只认**单列**合并（`startColumn == endColumn == col`）：横向跨度不是本判据的主题，
/// 混进来会把「列宽合并」误算成「分组合并」。
fn spans(book: &J, col: i64) -> Vec<i64> {
    let mut out: Vec<i64> = book["sheets"]["rpx-sheet-1"]["mergeData"]
        .as_array()
        .expect("workbook 必须有 mergeData")
        .iter()
        .filter(|m| m["startColumn"] == col && m["endColumn"] == col)
        .map(|m| m["endRow"].as_i64().unwrap() - m["startRow"].as_i64().unwrap() + 1)
        .collect();
    out.sort_unstable();
    out
}

/// 交叉表：分组格的跨度来自**纵向扩展**，`merge_same` 在这里是空转。
#[test]
fn crosstab_group_columns_merge_by_extension_span() {
    let book = render(&request("crosstab-merge-cells"));
    // 华东 2 家门店 × 2 个月 = 4 行；华南 1 家门店 × 2 个月 = 2 行。
    assert_eq!(
        spans(&book, 0),
        vec![2, 4],
        "大区列：华东跨 4 行、华南跨 2 行"
    );
    assert_eq!(spans(&book, 1), vec![2, 2, 2], "门店列：三家门店各跨 2 行");
    // 月份列取值 1月/2月/1月/2月…，相邻永不相等。
    assert!(
        spans(&book, 2).is_empty(),
        "月份列 1 月 / 2 月交替出现，一格都不许合并"
    );
}

/// 交叉表：同一份模板，只加数据不改模板，跨度必须跟着变 —— 「动态」二字的判据。
#[test]
fn crosstab_merge_span_follows_the_data() {
    let mut req = request("crosstab-merge-cells");
    let mut csv = String::from_utf8(req.datasets[DATA].clone()).unwrap();
    csv.push_str("华东,南京东路店,3月,空调,11000\n华东,南京东路店,3月,冰箱,7000\n");
    req.datasets.insert(DATA.to_string(), csv.into_bytes());
    let book = render(&req);
    assert_eq!(
        spans(&book, 0),
        vec![2, 5],
        "多出 3 月后，华东应从 4 行变 5 行"
    );
    assert_eq!(
        spans(&book, 1),
        vec![2, 2, 3],
        "南京东路店多一个月，应变成 3 行，其余两家仍 2 行"
    );
}

/// 明细表：这一列没有分组，跨度**只能**来自 `merge_same` —— 本文件里唯一有牙齿的判据。
#[test]
fn detail_merge_same_joins_adjacent_equal_cells() {
    let book = render(&request("detail-merge-same"));
    // 大区 / 门店仍是扩展跨度：华东 8 行（2 门店 × 4 条明细）、华南 4 行；门店各 4 行。
    assert_eq!(spans(&book, 0), vec![4, 8], "大区列由纵向扩展合并");
    assert_eq!(spans(&book, 1), vec![4, 4, 4], "门店列由纵向扩展合并");
    // 月份列没有分组，1月×2 + 2月×2 每家门店，共 6 段。
    assert_eq!(
        spans(&book, 2),
        vec![2, 2, 2, 2, 2, 2],
        "月份列由 merge_same 合并：每家门店 1 月、2 月各跨 2 行"
    );
    // 品类列空调/冰箱交替，相邻永不相等。
    assert!(spans(&book, 3).is_empty(), "品类列取值交替，不得合并");

    // 反证（自带故障注入）：摘掉 merge_same，同一份模板必须一格都不并。
    // 没有这一段的话，将来某个「反正也会合并」的实现能让上面那条断言继续假绿。
    let mut plain: J = serde_json::from_slice(&request("detail-merge-same").report).unwrap();
    for cell in plain["cells"].as_array_mut().unwrap() {
        cell.as_object_mut().unwrap().remove("merge_same");
    }
    let mut req = request("detail-merge-same");
    req.report = serde_json::to_vec(&plain).unwrap();
    let book = render(&req);
    assert!(
        spans(&book, 2).is_empty(),
        "摘掉 merge_same 后月份列不得合并（否则说明跨度另有来源）"
    );
    assert_eq!(
        spans(&book, 0),
        vec![4, 8],
        "分组格的扩展跨度与 merge_same 无关，摘掉后必须不变"
    );
}
