//! 「保险单明细表」的判据：序号列跨整块 + 明细合计 + 无空行。
//!
//! 这个例子的失效形态**都不会报错**：
//! ① 合计只加了一部分明细（引错数据集 / 漏列）—— 数字看着合理，就是少一截；
//! ② 序号列没跨满块（退回逐行）—— 报表仍然"成功"，只是版面变成重复的 1/1/1/1；
//! ③ **驱动格后面留了空模板行** —— 那些空行会被整块吃进去，报表照样成功，
//!    只是块里多出若干空行、合计行被推到很下面。这一条最难肉眼发现，故单列成测。
//!
//! 结果行号（0 基）：地址列表 4..6、第一部分明细 9..12、第一部分合计 13、
//! 第二部分明细 16..20、第二部分合计 21。
use rpx_engine::params::ParamTable;
use rpx_runtime::{execution::render_request, form::RenderRequest};
use serde_json::Value as J;
use std::{collections::BTreeMap, path::Path};

const AMOUNT: usize = 3; // 保险金额
const PREMIUM: usize = 5; // 保险费

fn render() -> J {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/r-model");
    let mut datasets = BTreeMap::new();
    for name in [
        "policy-address.csv",
        "policy-items.csv",
        "policy-items2.csv",
    ] {
        datasets.insert(name.to_string(), std::fs::read(dir.join(name)).unwrap());
    }
    let req = RenderRequest {
        report: std::fs::read(dir.join("insurance-policy.json")).unwrap(),
        report_name: "insurance-policy".into(),
        params: ParamTable::EMPTY,
        datasets,
    };
    serde_json::from_str(&render_request(&req).unwrap()).unwrap()
}

fn at(book: &J, row: usize, col: usize) -> f64 {
    book["sheets"]["rpx-sheet-1"]["cellData"][row.to_string()][col.to_string()]["v"]
        .as_f64()
        .unwrap_or_else(|| panic!("({row},{col}) 没有数值"))
}

#[test]
fn sequence_columns_span_their_whole_block() {
    let book = render();
    let mut spans: Vec<(i64, i64)> = book["sheets"]["rpx-sheet-1"]["mergeData"]
        .as_array()
        .expect("mergeData")
        .iter()
        .filter(|m| m["startColumn"] == 0 && m["endColumn"] == 0)
        .map(|m| {
            (
                m["startRow"].as_i64().unwrap(),
                m["endRow"].as_i64().unwrap(),
            )
        })
        .collect();
    spans.sort_unstable();
    // 地址标签跨 3 行、两部分序号各跨 4 / 5 行（0 基行号）。
    assert_eq!(spans, vec![(4, 6), (9, 12), (16, 20)]);
}

#[test]
fn both_parts_total_exactly_their_own_detail_rows() {
    let book = render();
    // 第一部分：明细在行 9..12，合计在 13。
    for col in [AMOUNT, PREMIUM] {
        let want: f64 = (9..=12).map(|r| at(&book, r, col)).sum();
        assert!(
            (at(&book, 13, col) - want).abs() < 1e-6,
            "第一部分合计 第 {col} 列：{want}"
        );
    }
    // 第二部分：明细在行 16..20，合计在 21。
    for col in [AMOUNT, PREMIUM] {
        let want: f64 = (16..=20).map(|r| at(&book, r, col)).sum();
        assert!(
            (at(&book, 21, col) - want).abs() < 1e-6,
            "第二部分合计 第 {col} 列：{want}"
        );
    }
    // 硬编码总数（来自 CSV 手算），钉住"加的是不是同一批数"。
    assert!((at(&book, 13, AMOUNT) - 590600.0).abs() < 1e-6);
    assert!((at(&book, 13, PREMIUM) - 119199.27).abs() < 1e-6);
    assert!((at(&book, 21, AMOUNT) - 1049145.0).abs() < 1e-6);
    assert!((at(&book, 21, PREMIUM) - 278158.80).abs() < 1e-6);
}

#[test]
fn no_blank_rows_are_eaten_into_the_detail_blocks() {
    let book = render();
    let rows = book["sheets"]["rpx-sheet-1"]["printRowCount"]
        .as_i64()
        .unwrap();
    assert_eq!(
        rows, 24,
        "24 行结果里不该有空行；多出来的说明模板空行被吃进块了"
    );
    let mut occupied = std::collections::HashSet::new();
    for (row, line) in book["sheets"]["rpx-sheet-1"]["cellData"]
        .as_object()
        .unwrap()
    {
        for (col, cell) in line.as_object().unwrap() {
            let has_value =
                cell["v"].as_f64().is_some() || cell["v"].as_str().is_some_and(|s| !s.is_empty());
            if has_value {
                occupied.insert((row.parse::<i64>().unwrap(), col.parse::<i64>().unwrap()));
            }
        }
    }
    // 第 3 行（0 基 2）是刻意留的间距行，其余从第 4 行起一行都不许空。
    for r in 3..rows {
        assert!(
            occupied.iter().any(|(row, _)| *row == r),
            "第 {} 行完全为空 —— 很可能是驱动格后面的模板空行被吃进了块",
            r + 1
        );
    }
}
