//! pager 支持性守卫的**端到端**验证：`calculate()` 对非默认分页配置显式报错。
//!
//! 单测（`src/pager/tests.rs`）钉 `ensure_supported` 本身；本文件钉两件事：
//!
//! ① **15 个夹具全在支持范围内** —— 夹具是产品 writer 产出的真字节，其
//!    `print_setup` 全部为默认（`pagerStyle=2` 单栏）。守卫若误伤夹具，
//!    这里先红，而不是等 `engine_grid_compare` 全量对拍时才以"莫名其妙的错误"暴露。
//! ② `calculate()` **真的调用**了守卫 —— 三个字段分别改一个后必须报
//!    `UnsupportedPaging`，且**在跑计算之前**（传空数据集也会先报这个错，
//!    而不是 `MissingDataset`）。
//!
//! 背景（`spec-m2.md` §6.2 带账项 / `m2-engine-design.md`）：本实现的分页只覆盖
//! `SimplePager._$5` 的 `pagerStyle==2 && columns==1` 分支（探针口径）；
//! 其余配置在 2026-10-14 前是**静默按 2 处理**，现改为显式失败。

mod common;

use common::all_names;
use rpx_engine::EngineError;
use std::path::PathBuf;

/// 夹具目录口径与 `tests/engine_grid_compare.rs:40-42` 一致。
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// 解析 `.rpx`（产品 writer 产出的真实字节），写法对齐 `tests/engine_grid_compare.rs:47-51`。
fn open_rpx(name: &str) -> rpx_core::ReportDefine {
    let path = fixtures_dir().join(format!("{name}.rpx"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("读 {} 失败：{e}", path.display()));
    rpx_core::open_bytes(&bytes).unwrap_or_else(|e| panic!("解析 {} 失败：{e}", path.display()))
}

/// ① 15 个夹具（产品真字节）的 `print_setup` 全部落在支持范围内。
///
/// 这是"守卫不误伤夹具"的直接证据；若未来某个新夹具用非默认分页配置，
/// 这里先红，逼出一次显式决策（补实现，或把该夹具改为豁免并有记录）。
#[test]
fn every_fixture_template_is_within_supported_paging() {
    for name in all_names() {
        let rd = open_rpx(name);
        assert!(
            rpx_engine::pager::ensure_supported(&rd.print_setup).is_ok(),
            "{name}: 夹具模板不在支持范围内 —— 守卫误伤 \
             (pager_style={}, column_mode={}, repeated_group_header={})",
            rd.print_setup.pager_style,
            rd.print_setup.column_mode,
            rd.print_setup.column_repeated_group_header,
        );
    }
}

/// ② `calculate()` 入口接守卫：三个字段分别改一个 ⇒ 必须报 `UnsupportedPaging`。
///
/// 守卫在计算之前 ⇒ 传**空数据集**也会先报 `UnsupportedPaging`；若守卫没生效，
/// 这条会以 `MissingDataset`（或其它错误）失败 —— 断言本身即区分二者。
/// 变异用例的可调用类型：把某个 `PrintSetup` 字段改成非默认值。
type Mutate = fn(&mut rpx_core::print_setup::PrintSetup);

#[test]
fn calculate_rejects_unsupported_paging_before_any_work() {
    let cases: [(&str, Mutate); 3] = [
        ("pagerStyle=1", |s| s.pager_style = 1),
        ("columnMode=1", |s| s.column_mode = 1),
        ("repeatedGroupHeader=true", |s| {
            s.column_repeated_group_header = true;
        }),
    ];
    for (label, mutate) in cases {
        let mut rd = open_rpx("engine_expr");
        mutate(&mut rd.print_setup);
        match rpx_engine::calculate(&rd, &[]) {
            Err(EngineError::UnsupportedPaging { .. }) => {}
            other => panic!("{label}: 期望 UnsupportedPaging，实际 {other:?}"),
        }
    }
}
