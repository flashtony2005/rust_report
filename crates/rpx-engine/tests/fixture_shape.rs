//! `expected.json` **顶层键**的结构与形态断言。
//!
//! # 为什么需要
//!
//! 顶层键目前只有 `expect_keys` 白名单兜着：它能挡住"**多了**键"和"**少了必填**键"，
//! 但挡不住"键还在、形态却不对"的情况（例如 `dataset.row_count` 与 `rows` 的实际长度
//! 对不上、`row_cells` 少了一行、`formulas` 的坐标跑到模板框外）。
//! 这些都是**产品 writer 的内部一致性**，属于 M2-09「结构存在且形态正确」的范围。
//!
//! # 分工
//!
//! * 本文件只管 `name` / `unit` / `dataset` / `template_*` / `formulas` / `row_cells`；
//! * `calculated` / `pagination` / `merged` 的形态分别由 `fixture_loader.rs`、
//!   `rowheight_baseline.rs`、`merged_assert.rs` 负责，**此处不重复**。
//!
//! # 纪律
//!
//! 期望值一律现读、**零转录**；遍历 `all_names()` 的**注册全集**，
//! 非错误型与错误型夹具都要过 —— 错误型夹具同样有完整的 `dataset`/`formulas`/`row_cells`。

mod common;

use common::{all_names, load};

/// ① `dataset` 三件套自洽：`row_count` == `rows.len()`；每行宽度 == `columns.len()`。
///
/// 这条挡住"writer 改了行数却没同步 `row_count`"这类静默漂移。
#[test]
fn dataset_row_count_and_row_width_agree_with_the_literal() {
    let mut seen = 0usize;
    for name in all_names() {
        let fx = load(name);
        assert!(!fx.dataset.columns.is_empty(), "{name}: columns 不该为空");
        assert_eq!(
            fx.dataset.row_count as usize,
            fx.dataset.rows.len(),
            "{name}: dataset.row_count 应等于 rows 的实际长度"
        );
        for (i, row) in fx.dataset.rows.iter().enumerate() {
            assert_eq!(
                row.len(),
                fx.dataset.columns.len(),
                "{name}: 第 {i} 行宽度应等于 columns 数量"
            );
        }
        seen += 1;
    }
    assert!(seen > 0, "没有遍历到任何夹具 ⇒ 本测试是永真式");
}

/// ② `template_rows` 与 `row_cells` 一一对应：`row_cells` 长度相等，
/// 且其 `r` 恰好是 `1..=template_rows`（1 基、连续、不重复）。
///
/// 依据 `GenEngineFixtures.java:147-155`：`for (int r = 1; r <= rd.getRowCount(); r++)`
/// 每行都写一条 `row_cells`，所以两者必须等长且行号连续。
#[test]
fn row_cells_cover_every_template_row_exactly_once() {
    let mut seen = 0usize;
    for name in all_names() {
        let fx = load(name);
        assert!(fx.template_rows >= 1, "{name}: template_rows 应为正");
        assert_eq!(
            fx.row_cells.len() as i32,
            fx.template_rows,
            "{name}: row_cells 长度应等于 template_rows"
        );
        let mut rows: Vec<i32> = fx.row_cells.iter().map(|rc| rc.r).collect();
        rows.sort_unstable();
        let want: Vec<i32> = (1..=fx.template_rows).collect();
        assert_eq!(
            rows, want,
            "{name}: row_cells 的 r 应恰好覆盖 1..=template_rows"
        );
        for rc in &fx.row_cells {
            assert!(
                rc.row_height > 0.0,
                "{name}: 行 {} 的 row_height 应为正",
                rc.r
            );
        }
        seen += 1;
    }
    assert!(seen > 0, "没有遍历到任何夹具 ⇒ 本测试是永真式");
}

/// ③ `formulas[]` 的坐标必须落在**模板框内**（1 基、`r <= template_rows`、`c <= template_cols`）。
///
/// 公式格是模板态概念，跑到扩展区之外就说明 writer 取错了坐标口径。
/// 错误型夹具也有 `formulas`（`engine_cycle` 有 2 条），所以这条对**全部**夹具成立。
#[test]
fn formula_coordinates_stay_inside_the_template_box() {
    let mut total = 0usize;
    for name in all_names() {
        let fx = load(name);
        for f in &fx.formulas {
            assert!(
                f.r >= 1 && f.r <= fx.template_rows,
                "{name}: 公式 ({},{}) 的行号越出模板 1..={}",
                f.r,
                f.c,
                fx.template_rows
            );
            assert!(
                f.c >= 1 && f.c <= fx.template_cols,
                "{name}: 公式 ({},{}) 的列号越出模板 1..={}",
                f.r,
                f.c,
                fx.template_cols
            );
            assert!(
                !f.exp.is_empty(),
                "{name}: 公式 ({},{}) 的 exp 不该为空串",
                f.r,
                f.c
            );
            total += 1;
        }
    }
    assert!(total > 0, "没有遍历到任何公式格 ⇒ 本测试是永真式");
}

/// ④ 非错误型夹具：计算态网格**不小于**模板（扩展只增不减）。
///
/// 错误型夹具没有 `calculated.rows`，本测试跳过它们（由 `merged_assert.rs` 覆盖其形态）。
#[test]
fn calculated_grid_is_never_smaller_than_the_template() {
    let mut seen = 0usize;
    for name in all_names() {
        let fx = load(name);
        // 错误型夹具没有网格，跳过（其形态由 merged_assert.rs 负责）
        let Some(g) = fx.grid() else { continue };
        assert!(
            g.rows >= fx.template_rows,
            "{name}: 计算态行数 {} 小于模板行数 {}",
            g.rows,
            fx.template_rows
        );
        assert!(
            g.cols >= fx.template_cols,
            "{name}: 计算态列数 {} 小于模板列数 {}",
            g.cols,
            fx.template_cols
        );
        seen += 1;
    }
    assert!(seen > 0, "没有任何带网格的夹具 ⇒ 本测试是永真式");
}
