//! 测试共享支撑：把产品产出的 `tests/fixtures/engine_*.expected.json`
//! 载入为类型化结构（[`model::Fixture`]）。
//!
//! # 为什么单独抽一层
//!
//! `docs/spec-m2.md` §7 的验收项 M2-09 要求计算态**逐格**断言（值 + 类型 +
//! `source` 的 6 个子字段）、分页**逐页**断言、合并**逐格**断言 —— "只比矩阵维度
//! 不算通过"。这些 `.expected.json` 是产品引擎自己跑出来的唯一事实源，
//! 不是手写期望（`GenEngineFixtures.java:1-28`）。没有这一层，引擎就无法被验收。
//!
//! # 为什么放行 dead_code / unused_imports
//!
//! Rust 的 integration test 各自把本模块编译一份：某个 test 只用到其中一部分，
//! 而 `mod common;` 是**私有**模块，未用到的 `pub` 项在测试二进制里不可从根部到达，
//! 会被 `dead_code` 判为未使用，在 `-D warnings` 下直接失败。故在此统一放行。
//! 这只影响测试支撑代码，不掩盖被测代码（`src/`）的任何问题。
#![allow(dead_code, unused_imports)]

pub mod compare;
pub mod json;
pub mod loader;
pub mod model;

pub use compare::{
    compare_actual_text, compare_actual_with_raw, compare_fixture_to_itself, Diff, Dim, Kind,
    Report,
};
pub use loader::{all_names, load, load_raw, parse_fixture};
pub use model::*;

/// 读取落盘的 JSON 定义（含 Preserve 快照），并还原合并延续独立格。
///
/// 引擎测试的主输入是 `tests/fixtures/format/engine/*.json`；`.rpx` 只留
/// 兼容/往返测试。合并区 A3 这类独立延续格由快照里的原始字节还原。
pub fn open_json_fixture(name: &str) -> rpx_core::ReportDefine {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/format/engine")
        .join(format!("{name}.json"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("读 {} 失败：{e}", path.display()));
    let (mut rd, _) = rpx_format::reportdef::open_report_full(&bytes)
        .unwrap_or_else(|e| panic!("解析 {} 失败：{e}", path.display()));
    restore_merge_continuations(&mut rd, &bytes);
    rd
}

fn restore_merge_continuations(rd: &mut rpx_core::ReportDefine, json: &[u8]) {
    let Ok(j) = serde_json::from_slice::<serde_json::Value>(json) else {
        return;
    };
    let Some(encoded) = j["extensions"]["rpx.legacy"]["data"].as_str() else {
        return;
    };
    use base64::{engine::general_purpose::STANDARD, Engine};
    let Ok(bytes) = STANDARD.decode(encoded) else {
        return;
    };
    let Ok(original) = rpx_core::open_bytes(&bytes) else {
        return;
    };
    if original.rows != rd.rows || original.cols != rd.cols {
        return;
    }
    for area in &original.merges {
        for r in area.begin_row..=area.end_row {
            for c in area.begin_col..=area.end_col {
                if r == area.begin_row && c == area.begin_col {
                    continue;
                }
                let Some(orig_id) = original.grid[r][c] else {
                    continue;
                };
                if Some(orig_id) == original.grid[area.begin_row][area.begin_col] {
                    continue;
                }
                let Some(cell) = original.cell_at(r, c) else {
                    continue;
                };
                let mut cell = cell.clone();
                cell.merged_area = None;
                let id = rd.store.len();
                rd.store.push(cell);
                rd.grid[r][c] = Some(id);
            }
        }
    }
}
