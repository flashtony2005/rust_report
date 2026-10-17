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
use common::open_json_fixture;

use common::all_names;
use rpx_engine::EngineError;
/// ① 15 个夹具（产品真字节）的 `print_setup` 全部落在支持范围内。
///
/// 这是"守卫不误伤夹具"的直接证据；若未来某个新夹具用非默认分页配置，
/// 这里先红，逼出一次显式决策（补实现，或把该夹具改为豁免并有记录）。
#[test]
fn every_fixture_template_is_within_supported_paging() {
    for name in all_names() {
        let rd = open_json_fixture(name);
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

/// ①'（M3-06）17 个夹具的模板**列区带**守卫不误伤：全部 `colType == -79` 数据列。
///
/// `ensure_data_columns` 若对夹具报错，说明夹具里有左右列头（`-80`/`-78`）——
/// 那时 `split_cols` 的全列假设就不成立，必须先补实现再收夹具。
#[test]
fn every_fixture_template_has_only_data_columns() {
    for name in all_names() {
        let rd = open_json_fixture(name);
        assert!(
            rpx_engine::pager::ensure_data_columns(&rd).is_ok(),
            "{name}: 夹具模板出现非数据列（列区带）—— split_cols 的 1..=cols 假设被打破"
        );
    }
}

/// ② `calculate()` 入口接守卫：四个字段分别改一个 ⇒ 必须报 `UnsupportedPaging`。
///
/// 守卫在计算之前 ⇒ 传**空数据集**也会先报 `UnsupportedPaging`；若守卫没生效，
/// 这条会以 `MissingDataset`（或其它错误）失败 —— 断言本身即区分二者。
/// 变异用例的可调用类型：把某个 `PrintSetup` 字段改成非默认值。
type Mutate = fn(&mut rpx_core::print_setup::PrintSetup);

#[test]
fn calculate_rejects_unsupported_paging_before_any_work() {
    let cases: [(&str, Mutate); 3] = [
        ("pagerStyle=3", |s| s.pager_style = 3),
        ("columnMode=1", |s| s.column_mode = 1),
        // M3-06：tableColumnNum > 1 走产品的多栏分支（PageBuilder.init1 →
        // pager.setColumns），本实现未移植 —— 显式拒绝。
        ("tableColumnNum=0", |s| s.table_column_num = 0),
    ];
    for (label, mutate) in cases {
        let mut rd = open_json_fixture("engine_expr");
        mutate(&mut rd.print_setup);
        match rpx_engine::calculate(&rd, &[]) {
            Err(EngineError::UnsupportedPaging { .. }) => {}
            other => panic!("{label}: 期望 UnsupportedPaging，实际 {other:?}"),
        }
    }
}

/// ③（M3-06）列区带变异：把一个夹具列改成左列头（`-80`）⇒ 必须报
/// `UnsupportedPaging`，证明 `ensure_data_columns` 真的接进了 `calculate()` 入口。
#[test]
fn calculate_rejects_column_bands_before_any_work() {
    let mut rd = open_json_fixture("engine_expr");
    assert!(
        !rd.col_cells.is_empty(),
        "engine_expr 应至少有一列（守卫变异的前提）"
    );
    rd.col_cells[0].col_type = -77; // IColCell.TYPE_LEFT_HEADER
    match rpx_engine::calculate(&rd, &[]) {
        Err(EngineError::UnsupportedPaging { detail }) => {
            assert!(
                detail.contains("colType=-77"),
                "错误应点名 colType，实得：{detail}"
            );
        }
        other => panic!("期望 UnsupportedPaging，实际 {other:?}"),
    }
}

/// ④（M3-06）`paginate_with_setup` 自带守卫复核：不经 `calculate()` 直接调也安全。
#[test]
fn paginate_with_setup_rechecks_the_guard() {
    let rd = open_json_fixture("engine_expr");
    let mut ps = rd.print_setup.clone();
    ps.pager_style = 3; // 单页短路分支未实现
    let grid = rpx_engine::arena::CalcGrid::new(1, 1);
    match rpx_engine::pager::paginate_with_setup(&grid, &ps) {
        Err(EngineError::UnsupportedPaging { .. }) => {}
        other => panic!("期望 UnsupportedPaging，实际 {other:?}"),
    }
}
