//! 「复杂格间计算」的判据：多块布局 + 块级格间引用。
//!
//! 这个例子的失效形态**都不会报错**：
//! ① 累计只加了其中一块（引错列 / 漏一块）—— 数字看着合理，就是小了一截；
//! ② 损失率除错分母（拿交单金额当入库金额）—— 百分比照样输出，只是没意义；
//! ③ 块标题没撑满 —— 版面塌成重复文字，报表仍然「成功」。
//!
//! ⛔ 块标题的跨度**不来自 `merge_same`**（2026-10-18 实测）：静态 `text` 格在扩展
//! 块里本来就跨整块，给它加 `merge_same` 是空转，去掉照样跨。真正会把它打回逐行的是
//! 给它挂 `left_head`（等于声明「我是逐行格」）—— 故障注入按这条验过：span 从
//! `[8,8,8]` 变成 `[8,8]`，判据变红。与交叉表分组格是同一类坑，别再加 `merge_same`。
//!
//! 所以判据同时钉三件事：**块跨度**、**块内合计**（从结果自己算）、
//! **跨块汇总**（累计 / 损失率 / 条件合计）。
//!
//! 结果布局（0 基，与 `mergeData` 一致）：块 1 = 行 3..10，块 2 = 行 11..18，
//! 块 3 = 行 19..26，汇总 = 行 27..30；F 列 = 第 5 列。
use rpx_engine::params::ParamTable;
use rpx_runtime::{execution::render_request, form::RenderRequest};
use serde_json::Value as J;
use std::{collections::BTreeMap, path::Path};

const F: usize = 5;
const C: usize = 2;

fn render() -> J {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/r-model");
    let req = RenderRequest {
        report: std::fs::read(dir.join("complex-cell-calc.json")).unwrap(),
        report_name: "complex-cell-calc".into(),
        params: ParamTable::EMPTY,
        datasets: BTreeMap::from([(
            "quality-loss.csv".into(),
            std::fs::read(dir.join("quality-loss.csv")).unwrap(),
        )]),
    };
    serde_json::from_str(&render_request(&req).unwrap()).unwrap()
}

fn sheet(book: &J) -> &J {
    &book["sheets"]["rpx-sheet-1"]
}

/// 结果里 `(row, col)` 的数值。
fn at(book: &J, row: usize, col: usize) -> f64 {
    sheet(book)["cellData"][row.to_string()][col.to_string()]["v"]
        .as_f64()
        .unwrap_or_else(|| panic!("({row},{col}) 没有数值"))
}

/// 某列一段行的合计（从结果自己加，不依赖汇总格）。
fn sum_rows(book: &J, rows: std::ops::Range<usize>, col: usize) -> f64 {
    rows.map(|r| at(book, r, col)).sum()
}

#[test]
fn block_labels_span_exactly_their_own_block() {
    let book = render();
    let mut spans: Vec<i64> = sheet(&book)["mergeData"]
        .as_array()
        .expect("mergeData")
        .iter()
        .filter(|m| m["startColumn"] == 0 && m["endColumn"] == 0)
        .map(|m| m["endRow"].as_i64().unwrap() - m["startRow"].as_i64().unwrap() + 1)
        .collect();
    spans.sort_unstable();
    // 8 个产品 × 3 块：每块标题整块合并。出现 1 说明 merge_same 没生效。
    assert_eq!(spans, vec![8, 8, 8], "三块标题各应跨 8 行");
    assert_eq!(
        sheet(&book)["printRowCount"],
        31,
        "3×8 明细 + 3 表头 + 4 汇总"
    );
}

#[test]
fn cross_block_total_equals_the_two_blocks_it_names() {
    let book = render();
    let loss = sum_rows(&book, 3..11, F); // 块 1：损失金额
    let extra = sum_rows(&book, 11..19, F); // 块 2：行内小计
    let inbound = sum_rows(&book, 19..27, C); // 块 3：入库金额
    assert!((loss - 35661.0).abs() < 1e-6, "块 1 合计 {loss}");
    assert!((extra - 17500.0).abs() < 1e-6, "块 2 合计 {extra}");
    assert!((inbound - 1783105.0).abs() < 1e-6, "块 3 合计 {inbound}");

    // 累计行 = 块 1 + 块 2（**不是**块 1 + 块 3，也不是只加一块）。
    assert!((at(&book, 27, F) - (loss + extra)).abs() < 1e-6, "累计损失");
    assert!((at(&book, 28, F) - inbound).abs() < 1e-6, "入库合计");
    // 损失率 = 累计 / 入库。除错分母时会输出 >100% 这种一眼假的数。
    let rate = at(&book, 29, F);
    assert!(
        (rate - (loss + extra) / inbound).abs() < 1e-9,
        "损失率 {rate}"
    );
    assert!((0.0..1.0).contains(&rate), "损失率应小于 100%：{rate}");
}

#[test]
fn conditional_total_skips_rows_below_the_threshold() {
    let book = render();
    // 块 2 小计 > 500：2300+770+4200+6900+1660+1330 = 17160；
    // 340 与 0 被排除。阈值写错成 >0 会得到 17500 —— 两者必须能区分开。
    assert!((at(&book, 30, F) - 17160.0).abs() < 1e-6, "条件合计");
    let all: f64 = sum_rows(&book, 11..19, F);
    assert!((all - 17500.0).abs() < 1e-6, "无条件合计");
    assert!(all > at(&book, 30, F), "条件合计必须小于全量合计");
}
