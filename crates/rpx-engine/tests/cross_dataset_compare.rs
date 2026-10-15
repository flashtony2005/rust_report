//! M5-02：**跨数据集关联**夹具对拍（T1/T2 非嵌套从格带 + T3 Root 态变体）。
//!
//! # 这条链路验收什么
//!
//! 对 7 个 cross 夹具各跑一遍：「`.rpx` 模板 + 内存 SQL 行集 →
//! `load_datasets_full(rd, read, Some(&kb_fetch))` → `calculate()` →
//! `emit::render_document()`」，与产品探针真值
//! `tests/fixtures/out/cross_*_product.json` **逐格**比（值/类型/`source`/
//! `sub_values`/head/尺寸/`merged.cells`/分页）。
//!
//! # 数据来源（为什么可以用内存行集）
//!
//! 模板里的 SQL 是固定的两条（`SELECT ... FROM report.emp_info/emp_bonus
//! ORDER BY id`），探针真值就是这两条 SQL 在金仓 `report` schema 上的产出
//! （spec-m5 §3：emp_info 5 行 / emp_bonus 6 行）。本文件把这两份行集写成
//! **内存复刻**（值域与真值 JSON 的 calculated 逐格一致，见下「复刻自检」），
//! 于是夹具对拍不依赖数据库可用性 —— 与 `engine_grid_compare` 的 CSV 夹具
//! 同纪律（真库冒烟归 `rpx-ds/smoke_pg.rs`，引擎 crate 零依赖）。
//!
//! # 与 M5-04 的关系
//!
//! M5-04 的正式夹具（`engine_cross*`，Java 侧 `GenEngineFixtures` 产出
//! `.expected.json`）落地后进 `engine_grid_compare` 主链；本文件先以探针
//! 真值把 T1/T2/T3 的语义钉住，两条链共用 [`common::compare`] 同一套口径。

mod common;

use common::{compare_actual_with_raw, Dim, Kind};
use rpx_core::rowset::RowSet;
use rpx_core::value::Value;
use rpx_engine::dataset::{load_datasets_full, SqlFetcher};
use rpx_engine::error::EngineError;

/// 夹具目录（与 `engine_grid_compare.rs` 同口径）。
fn fixtures_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn open_rpx(name: &str) -> rpx_core::ReportDefine {
    let path = fixtures_dir().join(format!("{name}.rpx"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("读 {} 失败：{e}", path.display()));
    rpx_core::open_bytes(&bytes).unwrap_or_else(|e| panic!("解析 {} 失败：{e}", path.display()))
}

/// 读探针真值原始 JSON（`out/cross_<name>_product.json`）。
fn truth_raw(name: &str) -> common::json::Json {
    let path = fixtures_dir().join(format!("out/{name}_product.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("读真值 {} 失败：{e}", path.display()));
    common::json::parse(&text).unwrap_or_else(|e| panic!("真值不是合法 JSON：{e}"))
}

// ─────────────────────────────────────────────────────────────
// 金仓 report schema 的内存复刻（spec-m5 §3 探针数据）
// ─────────────────────────────────────────────────────────────

/// `report.emp_info`（SQL: `SELECT id, dept, name, salary ... ORDER BY id`）。
fn emp_info() -> RowSet {
    RowSet::new(
        "ds1",
        &["id", "dept", "name", "salary"],
        vec![
            vec![
                Value::Int(1),
                Value::Text("人事部".into()),
                Value::Text("张伟".into()),
                Value::Int(8500),
            ],
            vec![
                Value::Int(2),
                Value::Text("财务部".into()),
                Value::Text("李娜".into()),
                Value::Int(9200),
            ],
            vec![
                Value::Int(3),
                Value::Text("技术部".into()),
                Value::Text("王强".into()),
                Value::Int(12000),
            ],
            vec![
                Value::Int(4),
                Value::Text("技术部".into()),
                Value::Text("赵敏".into()),
                Value::Int(11000),
            ],
            vec![
                Value::Int(5),
                Value::Text("市场部".into()),
                Value::Text("孙丽".into()),
                Value::Int(7800),
            ],
        ],
    )
}

/// `report.emp_bonus`（SQL: `SELECT id, emp_id, item, amount ... ORDER BY id`）。
/// 张伟 2 项 / 李娜 1 / 王强 1 / 赵敏 2 / **孙丽 0**（无匹配用例）。
fn emp_bonus() -> RowSet {
    RowSet::new(
        "ds2",
        &["id", "emp_id", "item", "amount"],
        vec![
            vec![
                Value::Int(1),
                Value::Int(1),
                Value::Text("全勤奖".into()),
                Value::Int(500),
            ],
            vec![
                Value::Int(2),
                Value::Int(1),
                Value::Text("年终奖".into()),
                Value::Int(3000),
            ],
            vec![
                Value::Int(3),
                Value::Int(2),
                Value::Text("全勤奖".into()),
                Value::Int(500),
            ],
            vec![
                Value::Int(4),
                Value::Int(3),
                Value::Text("餐补".into()),
                Value::Int(300),
            ],
            vec![
                Value::Int(5),
                Value::Int(4),
                Value::Text("项目奖".into()),
                Value::Int(2000),
            ],
            vec![
                Value::Int(6),
                Value::Int(4),
                Value::Text("全勤奖".into()),
                Value::Int(500),
            ],
        ],
    )
}

/// 内存 `SqlFetcher`：按 SQL 文本里的表名路由（模板两条 SQL 固定，
/// `emp_info` → ds1、`emp_bonus` → ds2，2026-10-15 dump 实证）。
///
/// 为什么按表名而不按数据集名路由：[`load_datasets_full`] 只把
/// `(dsn, sql)` 交给闭包，数据集名（`ds1`/`ds2`）在它自己手里 ——
/// 闭包能拿到的只有 SQL 文本。返回 [`EngineError`] 的形状与 rpx-ds
/// 真库路径同构（连接/SQL 失败原样上抛）。
fn kb_fetch(dsn: &str, sql: &str) -> std::result::Result<RowSet, EngineError> {
    if sql.contains("emp_info") {
        Ok(emp_info())
    } else if sql.contains("emp_bonus") {
        Ok(emp_bonus())
    } else {
        Err(EngineError::SqlDatasetNoConnection {
            name: "<unknown>".into(),
            dsn: dsn.to_string(),
        })
    }
}

/// 跑一个 cross 夹具，产出引擎侧文本。
fn engine_text(name: &str) -> String {
    let rd = open_rpx(name);
    let fetch: &SqlFetcher = &kb_fetch;
    let ds = load_datasets_full(&rd, &|_| None, Some(&fetch))
        .unwrap_or_else(|e| panic!("{name}: load_datasets_full 失败：{e}"));
    match rpx_engine::calculate(&rd, &ds) {
        Ok(grid) => rpx_engine::emit::render_document(&grid),
        Err(e) => panic!("{name}: calculate 失败：{e}"),
    }
}

// ─────────────────────────────────────────────────────────────
// 对拍
// ─────────────────────────────────────────────────────────────

/// 10 个 cross 夹具逐一与探针真值逐格比（T1/T2 + T3 六变体 + T4/T5 嵌套主子表）。
///
/// 覆盖（spec-m5 §3）：T1 非嵌套从格带（过滤引用主格 A3）、T2 去重主格 +
/// 同数据集从格、T3 六变体（过滤/限量/去重/降序/裸列/计数）。
/// T4/T5 嵌套主子表走 M5-03/M5-04，不在本文件（未实现即未验收）。
#[test]
fn cross_datasets_match_product_truth_cell_by_cell() {
    let names = [
        "cross_emp_bonus",     // T1
        "cross_dept_band",     // T2
        "cross_root_filter",   // T3 过滤
        "cross_root_num",      // T3 限量
        "cross_root_distinct", // T3 去重
        "cross_root_desc",     // T3 降序
        "cross_root_bare",     // T3 裸列
        "cross_root_count",    // T3 计数
        "cross_nest",          // T4（M5-03 嵌套主子表，显式左主格）
        "cross_nest_vals",     // T5（M5-03 嵌套主子表，无匹配仍 1 行）
    ];
    for name in names {
        let report = compare_actual_with_raw(&engine_text(name), &truth_raw(name), name);

        // ① 网格尺寸必须两侧一致（尺寸差异不在豁免之列）。
        assert_eq!(
            report.expected_dims,
            report.actual_dims,
            "{name}: 计算态网格尺寸不一致\n{}",
            report.summary()
        );

        // ② 非空跑钉子：真值网格非空，否则"干净"可能是空跑。
        let raw = truth_raw(name);
        let cells = raw
            .get("calculated")
            .and_then(|c| c.get("cells"))
            .expect("真值必有 calculated.cells");
        assert!(
            !matches!(cells, common::json::Json { kind: common::json::Kind::Arr(a), .. } if a.is_empty()),
            "{name}: 真值网格为空，'干净'是空跑"
        );

        // ③ 全部差异均为阻断项（含扩展结构 missing/extra）。
        for d in &report.diffs {
            let expensive = matches!(d.dim, Dim::Value | Dim::Type | Dim::Source)
                && matches!(d.kind, Kind::Missing | Kind::Extra);
            assert!(!expensive, "{name}: 扩展结构差异：{d}");
        }
        assert!(
            report.diffs.is_empty(),
            "夹具 {name} 必须逐格、逐页零差异\n{}",
            report.summary()
        );
    }
}

// ─────────────────────────────────────────────────────────────
// 复刻自检：内存行集 vs 真值 JSON 的 dataset 无关断言
// ─────────────────────────────────────────────────────────────

/// 复刻纪律：内存行集的关键事实必须与 spec-m5 §3 文字记载一致
/// （探针真值 JSON 不含原始行集 —— SQL 直连的产物只落计算态）。
/// 这条钉子防的是「改复刻数据去迁就对拍失败」。
#[test]
fn memory_replica_matches_spec_facts() {
    let info = emp_info();
    assert_eq!(info.rows.len(), 5, "emp_info 5 人（spec-m5 §3）");
    assert_eq!(
        info.rows[0][2],
        Value::Text("张伟".into()),
        "首行张伟（T1 主格第一实例）"
    );
    assert_eq!(info.rows[0][0], Value::Int(1), "张伟 id=1");
    let bonus = emp_bonus();
    assert_eq!(bonus.rows.len(), 6, "emp_bonus 6 行");
    // 张伟 2 项、孙丽 0 项（T1/T4 的多匹配/无匹配用例）
    assert_eq!(
        bonus.rows.iter().filter(|r| r[1] == Value::Int(1)).count(),
        2
    );
    assert_eq!(
        bonus.rows.iter().filter(|r| r[1] == Value::Int(5)).count(),
        0
    );
    // 行序 = ORDER BY id（过滤/排序变体的首现序基准）
    for (i, r) in bonus.rows.iter().enumerate() {
        assert_eq!(r[0], Value::Int(i as i32 + 1), "emp_bonus 按 id 升序");
    }
}
