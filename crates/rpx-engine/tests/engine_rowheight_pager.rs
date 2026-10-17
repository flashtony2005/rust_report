//! M2-22 行属性表达式的端到端基准：产品计算态行高必须驱动分页。
//!
//! `engine_rowheight` 含三条行高表达式（常量、乘法、加法），其中扩展锚点行
//! 会克隆 12 次。这里把运行结果与产品夹具保留的 17 个计算态行高对拍，
//! 再断言分页切点 8+9 行；总对拍器也要求该夹具全段零差异、无任何豁免。

mod common;
use common::open_json_fixture;

use std::path::PathBuf;

use common::json;
use common::Fixture;

/// 夹具目录口径与 `tests/engine_grid_compare.rs:41-43` 一致。
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// 引擎跑该夹具，返回 `(网格, 渲染文本)`。
fn run(name: &str) -> (rpx_engine::CalcGrid, String) {
    let rd = open_json_fixture(name);
    let ds = rpx_engine::dataset::load_datasets(&fixtures_dir(), &rd)
        .unwrap_or_else(|e| panic!("{name}: load_datasets 失败：{e}"));
    let grid = rpx_engine::calculate(&rd, &ds).expect("engine_rowheight 应算出网格");
    let text = rpx_engine::emit::render_document(&grid);
    (grid, text)
}

/// 夹具 `row_cells[].extra` 里的 `computed_row_heights`（产品算出的**计算态**行高）。
///
/// 走"加载器保存原文 → 用同一个 JSON reader 读回"，与 `tests/rowheight_baseline.rs:46-64`
/// 同一路径。
fn computed_heights(fx: &Fixture) -> Vec<f32> {
    let raw = fx
        .row_cells
        .iter()
        .filter_map(|rc| {
            rc.extra
                .iter()
                .find(|(k, _)| k == "computed_row_heights")
                .map(|(_, v)| v.as_str())
        })
        .next()
        .expect("engine_rowheight 的 row_cells 里应带 computed_row_heights");
    json::parse(raw)
        .unwrap_or_else(|e| panic!("computed_row_heights 原文解析失败：{e}"))
        .arr()
        .iter()
        .map(|j| j.as_f64() as f32)
        .collect()
}

/// RowCell key 1 expressions are evaluated and expanded to every runtime row.
#[test]
fn row_attribute_expressions_compute_expanded_row_heights() {
    let (grid, _) = run("engine_rowheight");
    let fx = common::load("engine_rowheight");
    let expected = computed_heights(&fx);

    assert_eq!(grid.row_heights, expected);
    assert_eq!(grid.row_heights.len() as i32, grid.rows);
    assert_eq!(
        grid.row_heights,
        vec![
            10.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 50.0,
            27.0, 10.0, 10.0
        ],
        "常量、乘法、加法和扩展克隆都必须写回计算态行高"
    );
}

/// Computed row-height values must be consumed by the pager.
#[test]
fn computed_row_heights_drive_the_expected_page_cut() {
    let (grid, _) = run("engine_rowheight");
    let fx = common::load("engine_rowheight");
    let expected = fx.pagination.as_ref().expect("产品夹具必须有分页");
    let actual = grid.pagination.as_ref().expect("引擎必须产出分页");

    assert_eq!(actual.page_count, expected.page_count);
    let rows: Vec<i32> = actual.pages.iter().map(|page| page.rows).collect();
    assert_eq!(rows, vec![8, 9]);
    assert_eq!(
        rows,
        expected
            .pages
            .iter()
            .map(|page| page.rows)
            .collect::<Vec<_>>()
    );
    assert_eq!(grid.row_heights[..8].iter().sum::<f32>(), 290.0);
    assert_eq!(grid.row_heights[8..].iter().sum::<f32>(), 297.0);
}

/// There is no longer a deferred diff or a fixture-level exception.
#[test]
fn rowheight_fixture_has_no_remaining_output_diffs() {
    let (_, text) = run("engine_rowheight");
    let report = common::compare_actual_text(&text, "engine_rowheight");
    assert!(
        report.diffs.is_empty(),
        "M2-22 should close all rowheight differences\n{}",
        report.summary()
    );
}
