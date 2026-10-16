//! SQLite 数据集端到端（用户问题「可不可以接 SQLite 运行报表」的**机器判据**，
//! 2026-10-16 冒烟实证后固化）。
//!
//! # 与 `sql_fixture.rs`（金仓）的区别
//!
//! **零外部依赖、恒真跑**（无 env 门控）：SQLite 是嵌入式库，测试用
//! **内存库**（`sqlite::memory:`）自建表，不依赖任何服务器与文件。
//!
//! # URL 形态实证矩阵（`rpx-ds/tests/url_probe.rs`，2026-10-16，sqlx Any 层）
//!
//! | 形态 | connect+fetch | 备注 |
//! | --- | --- | --- |
//! | `sqlite::memory:` | ✅ | 内存库；**单连接池**（`sql_source.rs` 的 sqlite 分支）⇒ 建表与取数同库 |
//! | `sqlite:{abs 反斜杠}` / `sqlite:///{abs 反斜杠}` | ✅ | 文件库；HTTP 冒烟同款（`tests/fixtures/out/sqlite_datasources.json`） |
//! | `sqlite://{abs}`（两斜杠） | ❌ connect 过、fetch code 14 | host 被剥，路径错位 |
//! | 相对路径 / `sqlite://:memory:` | ❌ code 14 / empty host | |
//!
//! ⚠ 文件库 + Windows 的组合建议走 `--datasources` 给**与 server 同盘符的
//! 绝对反斜杠路径**（cwd 盘符无关）；跨盘/相对形态未实证可行。

use rpx_core::value::Value;
use rpx_ds::SqlSource;
use rpx_server::reportdef;

/// 建**内存库** + `emp` 表（3 行），返回取数源。
///
/// 单连接池（`sql_source.rs` 的 sqlite 分支 `max_connections=1`）⇒
/// `execute` 建表与 `fetch_sql` 取数天然同库（内存库随连接生死）。
async fn setup() -> SqlSource {
    let src = SqlSource::connect("sqlite::memory:")
        .await
        .expect("sqlite 内存库连接失败");
    src.execute("CREATE TABLE emp (id INTEGER, name TEXT, dept TEXT, salary REAL)")
        .await
        .expect("建表失败");
    src.execute("INSERT INTO emp VALUES (1, '张伟', '人事部', 8500.0)")
        .await
        .expect("插行失败");
    src.execute("INSERT INTO emp VALUES (2, '李娜', '财务部', 9200.0)")
        .await
        .expect("插行失败");
    src.execute("INSERT INTO emp VALUES (3, '王强', '技术部', 12000.0)")
        .await
        .expect("插行失败");
    src
}

/// 模板：与 `tests/fixtures/out/sqlite_demo.json`（HTTP 冒烟夹具）同构 ——
/// 表头两格字面量 + 纵带 `ds1.select(name/salary)`。
const DEF_JSON: &str = r#"{
  "version": 1,
  "grid": { "rows": 3, "cols": 2 },
  "cells": [
    { "row": 1, "col": 1, "text": "姓名" },
    { "row": 1, "col": 2, "text": "工资" },
    { "row": 2, "col": 1, "expr": "ds1.select(name)" },
    { "row": 2, "col": 2, "expr": "ds1.select(salary)" }
  ],
  "datasets": [
    { "name": "ds1", "sql": { "dsn": "demo", "sql": "SELECT name, salary FROM emp ORDER BY id" } }
  ]
}"#;

/// 全链路：定义 JSON → build → SQLite 物化 → 引擎扩展计算 → 逐格断言。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sqlite_dataset_drives_the_report_end_to_end() {
    let src = setup().await;
    let rd = reportdef::build(DEF_JSON).expect("定义 JSON 应被接受");
    assert_eq!(rd.datasets.len(), 1);
    let sql_cfg = rd.datasets[0].sql.as_ref().expect("SQL 配置已解析");
    assert_eq!(sql_cfg.data_source_name.as_deref(), Some("demo"));

    // SQL 预取桥（`sql_fixture.rs` 金仓测试同款：async 取数桥进同步委托
    // 闭包，多线程 runtime + block_in_place）。
    let datasets = rpx_engine::dataset::load_datasets_full(
        &rd,
        &|_| None,
        Some(&|dsn, sql| {
            assert_eq!(dsn, "demo", "数据源名来自模板引用");
            assert!(sql.contains("FROM emp"), "模板 SQL 原样透传：{sql}");
            let fut = src.fetch_sql(sql);
            tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(fut)).map_err(
                |e| rpx_engine::EngineError::SqlFetch {
                    detail: e.to_string(),
                },
            )
        }),
    )
    .expect("数据集装载失败");
    assert_eq!(datasets.len(), 1);
    assert_eq!(datasets[0].name, "ds1");
    assert_eq!(datasets[0].cols, vec!["name", "salary"]);

    let grid = rpx_engine::calculate(&rd, &datasets).expect("计算失败");

    // 逐格断言（值 + 类型）：
    //   (1,1) 姓名   (1,2) 工资           ← 表头（Text）
    //   (2,1) 张伟   (2,2) 8500（Double） ← SQLite REAL → Double
    //   (3,1) 李娜   (3,2) 9200
    //   (4,1) 王强   (4,2) 12000          ← 纵带 3 行扩展，计算态 4 行
    let cell = |r: i32, c: i32| -> Value {
        grid.cells
            .iter()
            .find(|x| x.row == r && x.col == c)
            .unwrap_or_else(|| panic!("计算网格缺 ({r},{c})"))
            .value
            .clone()
    };
    assert_eq!(cell(1, 1), Value::Text("姓名".into()));
    assert_eq!(cell(1, 2), Value::Text("工资".into()));
    assert_eq!(cell(2, 1), Value::Text("张伟".into()));
    assert_eq!(cell(2, 2), Value::Double(8500.0));
    assert_eq!(cell(3, 1), Value::Text("李娜".into()));
    assert_eq!(cell(3, 2), Value::Double(9200.0));
    assert_eq!(cell(4, 1), Value::Text("王强".into()));
    assert_eq!(cell(4, 2), Value::Double(12000.0));
    // 计算态最大行 = 表头(1) + 3 数据行(2..4) = 4。
    let max_row = grid.cells.iter().map(|c| c.row).max().unwrap_or(0);
    assert_eq!(max_row, 4, "表头 1 行 + 3 数据行");
}
