//! `engine_sql` 金仓真机对拍（`spec-m3.md` M3-03 的 SQL 数据集链路清偿）。
//!
//! # 门控协议（同 `rpx-ds` 的 `tests/smoke_pg.rs`）
//!
//! 设 `RPX_KB_URL` / `RPX_KB_USER` / `RPX_KB_PASS` 才真跑；未设打印 SKIP 即过
//! （G3 在无金仓环境保持全绿）。默认目标 = 公司服务器 192.168.1.77:54323 的
//! KingbaseES（探针 `ProbeSql.java` 已在 `test` 库建好 `rpx_sql_smoke` 表——
//! 本测试**自带重建**（先删后建纪律的镜像），不依赖探针运行过）。
//!
//! # 对拍口径
//!
//! 产品真值 = `tests/fixtures/engine_sql.expected.json`（探针真机产出，
//! 回读自校验过）。Rust 侧：`.rpx` 解析 → `rpx-ds::SqlSource` 同 SQL 取数
//! → `DataSet::from_rowset` → `calculate` → 逐格对拍（值 + 类型）。
//! 全链路 = rpx-core（模板）× rpx-ds（取数）× rpx-engine（计算）三 crate。

use std::fs::File;

use rpx_core::value::Value;
use rpx_ds::SqlSource;

fn kb() -> Option<String> {
    std::env::var("RPX_KB_URL").ok().filter(|s| !s.is_empty())
}

const SQL_TABLE: &str = "rpx_sql_smoke";

/// 两条金仓测试共用同一张表（模板 SQL 里表名是写死的）⇒ 全程互斥——
/// 并行 DROP/CREATE 会在金仓目录索引上撞 23505（实测）。
static KB_TABLE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_sql_matches_the_product_ground_truth() {
    let _tbl = KB_TABLE_LOCK.lock().await;
    let Some(url) = kb() else {
        eprintln!("[sql_fixture] SKIP：未设 RPX_KB_URL（无金仓环境默认跳过）");
        return;
    };

    // ① 表自建（幂等）：与探针同数据。
    {
        let src = SqlSource::connect(&url).await.unwrap();
        src.fetch_sql("DROP TABLE IF EXISTS rpx_sql_smoke")
            .await
            .unwrap();
        src.fetch_sql(
            "CREATE TABLE rpx_sql_smoke (\
             id INTEGER PRIMARY KEY, dept VARCHAR(20), salary INTEGER)",
        )
        .await
        .unwrap();
        for (id, dept, salary) in [
            (1, "HR", 8500),
            (2, "FIN", 9200),
            (3, "DEV", 12000),
            (4, "DEV", 11000),
            (5, "MKT", 7800),
        ] {
            src.fetch_sql(&format!(
                "INSERT INTO rpx_sql_smoke VALUES ({id}, '{dept}', {salary})"
            ))
            .await
            .unwrap();
        }
    }

    // ② 模板解析（rpx-core）：SQL 数据集配置被读出，数据源名是引用。
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let rd = rpx_core::open(File::open(dir.join("engine_sql.rpx")).unwrap()).unwrap();
    assert_eq!(rd.datasets.len(), 1);
    let sql_cfg = rd.datasets[0].sql.as_ref().expect("SQL 配置已解析");
    assert_eq!(sql_cfg.data_source_name.as_deref(), Some("kingbase"));

    // ③ 取数（rpx-ds）→ 物化：走 `load_datasets_full` 完整管线（与产品链路
    // 同构：模板声明 → 数据源名 → SQL 透传 → `DataSet` 命名物化）。
    // async 取数桥进同步委托闭包：多线程 runtime + block_in_place。
    let src = SqlSource::connect(&url).await.unwrap();
    let datasets = rpx_engine::dataset::load_datasets_full(
        &rd,
        &|_| None,
        Some(&|dsn, sql| {
            assert_eq!(dsn, "kingbase", "数据源名来自模板引用");
            assert!(sql.contains(SQL_TABLE), "模板 SQL 原样透传：{sql}");
            let fut = src.fetch_sql(sql);
            tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(fut)).map_err(
                |e| rpx_engine::EngineError::SqlFetch {
                    detail: e.to_string(),
                },
            )
        }),
    )
    .unwrap();
    assert_eq!(datasets.len(), 1);
    assert_eq!(datasets[0].name, "ds1");
    assert_eq!(datasets[0].cols, vec!["id", "dept", "salary"]);

    // ④ 引擎计算。
    let grid = rpx_engine::calculate(&rd, &datasets).expect("计算");

    // ⑤ 逐格对拍产品真值（expected.json 的 calculated.cells：值 + 类型名）。
    let raw = std::fs::read_to_string(dir.join("engine_sql.expected.json")).unwrap();
    let want: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let cells = want["calculated"]["cells"].as_array().expect("cells 数组");
    assert!(!cells.is_empty());
    for w in cells {
        let (tr, tc) = (
            w["r"].as_i64().unwrap() as i32,
            w["c"].as_i64().unwrap() as i32,
        );
        let got = grid
            .cells
            .iter()
            .find(|c| c.row == tr && c.col == tc)
            .unwrap_or_else(|| panic!("计算网格缺 ({tr},{tc})"));
        let want_v = &w["value"];
        // 产品类型名 → Value 变体口径：int → Int、string → Text（loader 同款）。
        let ty = w["type"].as_str().unwrap_or("");
        match (ty, want_v) {
            ("string", serde_json::Value::String(s)) => {
                assert_eq!(got.value, Value::Text(s.clone()), "({tr},{tc}) 文本值");
            }
            ("int", serde_json::Value::Number(n)) => {
                let i = n.as_i64().expect("int 值");
                // 整数族在引擎里按字面量/物化收敛到 Int（金仓 int4 → Long 的
                // Any 层统一在 rpx-ds 侧；进入引擎后 Sum/引用语义不区分）。
                assert!(
                    got.value == Value::Int(i as i32) || got.value == Value::Long(i),
                    "({tr},{tc}) 整数值：got={:?} want={i}",
                    got.value
                );
            }
            other => panic!("未处理的类型形态 ({tr},{tc}): {other:?}"),
        }
    }
    // 扩展形状对齐产品真值口径：表头(1) + 5 名员工(2..6) ⇒ 计算态最大行 6。
    let max_r = grid.cells.iter().map(|c| c.row).max().unwrap_or(0);
    assert_eq!(max_r, 6, "扩展后最大行号（表头 1 + 5 数据行）");
    eprintln!("[sql_fixture] OK：金仓真机对拍逐格一致");
}

// ---- HTTP 级：数据源注册表 × `/render` 主链路（M3-05 的 SQL 延伸）----
//
// 三条：无注册表 400（点名 dsn 与补救路径）/ 未登记 dsn 400（零环境，sqlite
// 内存库当注册项）/ 金仓真机 HTTP 端到端（响应体与库函数直调逐字节一致）。

use std::sync::Arc;

use rpx_server::datasource::DataSourceRegistry;
use rpx_server::http::{app, render_request_ds};

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("读夹具失败 {}：{e}", p.display()))
}

/// 拼 multipart 体（单字段版够用：SQL 数据集模板不带 CSV）。
fn multipart_report(filename: &str, bytes: &[u8]) -> (Vec<u8>, String) {
    let b = "rpx-sql-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{b}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"report\"; filename=\"{filename}\"\r\n\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{b}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={b}"))
}

/// 起服务（ephemeral 端口；注册表由调用方给——None = 未配置）。
async fn spawn(dsr: Option<Arc<DataSourceRegistry>>) -> String {
    let store = Arc::new(rpx_ds::AssetStore::open(":memory:").await.unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app(store, dsr)).await.unwrap();
    });
    format!("127.0.0.1:{port}")
}

/// 真 socket POST /render → (状态码, 响应体)。
async fn post_render(addr: &str, body: Vec<u8>, ct: &str) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    let head = format!(
        "POST /render HTTP/1.1\r\nHost: {addr}\r\nContent-Type: {ct}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).await.unwrap();
    s.write_all(&body).await.unwrap();
    let mut resp = Vec::new();
    s.read_to_end(&mut resp).await.unwrap();
    let text = String::from_utf8_lossy(&resp).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|x| x.parse().ok())
        .unwrap_or(0);
    let body_text = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body_text)
}

#[tokio::test]
async fn sql_dataset_without_registry_is_400() {
    // 未给 --datasources：模板含 SQL 数据集 ⇒ 400，点名 dsn 与补救路径。
    let addr = spawn(None).await;
    let (body, ct) = multipart_report("engine_sql.rpx", &fixture("engine_sql.rpx"));
    let (status, resp) = post_render(&addr, body, &ct).await;
    assert_eq!(status, 400, "响应体：{resp}");
    assert!(resp.contains("kingbase"), "应点名数据源名：{resp}");
    assert!(resp.contains("--datasources"), "应给补救路径：{resp}");
}

#[tokio::test]
async fn sql_dataset_unknown_dsn_is_400() {
    // 注册表存在但 dsn 未登记：零环境（sqlite 内存库当注册项）。
    let reg = Arc::new(
        DataSourceRegistry::from_json(r#"{"other": "sqlite::memory:"}"#)
            .await
            .unwrap(),
    );
    let addr = spawn(Some(reg)).await;
    let (body, ct) = multipart_report("engine_sql.rpx", &fixture("engine_sql.rpx"));
    let (status, resp) = post_render(&addr, body, &ct).await;
    assert_eq!(status, 400, "响应体：{resp}");
    assert!(resp.contains("数据源未登记：kingbase"), "{resp}");
    assert!(resp.contains("other"), "应列出已登记数据源名：{resp}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn render_http_sql_dataset_matches_library() {
    let _tbl = KB_TABLE_LOCK.lock().await;
    let Some(url) = kb() else {
        eprintln!("[sql_fixture] SKIP（HTTP 级）：未设 RPX_KB_URL");
        return;
    };
    // 表自建（与上方直调级同数据）。
    let src = SqlSource::connect(&url).await.unwrap();
    src.fetch_sql("DROP TABLE IF EXISTS rpx_sql_smoke")
        .await
        .unwrap();
    src.fetch_sql(
        "CREATE TABLE rpx_sql_smoke (\
         id INTEGER PRIMARY KEY, dept VARCHAR(20), salary INTEGER)",
    )
    .await
    .unwrap();
    for (id, dept, salary) in [
        (1, "HR", 8500),
        (2, "FIN", 9200),
        (3, "DEV", 12000),
        (4, "DEV", 11000),
        (5, "MKT", 7800),
    ] {
        src.fetch_sql(&format!(
            "INSERT INTO rpx_sql_smoke VALUES ({id}, '{dept}', {salary})"
        ))
        .await
        .unwrap();
    }
    // 注册表：dsn=kingbase → 金仓连接（JSON 字符串形态 = 完整连接串）。
    let reg = Arc::new(
        DataSourceRegistry::from_json(&format!(r#"{{"kingbase": "{url}"}}"#))
            .await
            .unwrap(),
    );
    let addr = spawn(Some(reg.clone())).await;

    let req = rpx_server::form::RenderRequest {
        report: fixture("engine_sql.rpx"),
        report_name: "engine_sql".to_string(),
        datasets: Default::default(),
        params: rpx_engine::params::ParamTable::EMPTY,
    };
    // 库函数直调（对拍基准）。
    let lib = render_request_ds(&req, Some(&reg)).await.unwrap();
    // HTTP 面同输入逐字节一致（主链路 ①..⑩ 对拍纪律同 render_http.rs）。
    let (body, ct) = multipart_report("engine_sql.rpx", &fixture("engine_sql.rpx"));
    let (status, resp) = post_render(&addr, body, &ct).await;
    assert_eq!(status, 200, "响应体：{resp}");
    assert_eq!(resp, lib, "HTTP 响应体 ≡ 库函数直调");
    // 内容抽验：SQL 取数真的流到了快照（列名/行值/聚合列）。
    for s in ["id", "dept", "salary", "HR", "MKT", "8500", "7800"] {
        assert!(resp.contains(s), "快照应含 {s}");
    }
    eprintln!("[sql_fixture] OK：金仓 HTTP 端到端逐字节一致");
}
