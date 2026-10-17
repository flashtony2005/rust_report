//! 真实 Excel、带引号 CSV 与两个独立 SQLite 数据库经生产入口计算。
mod common;
use rpx_engine::params::ParamTable;
use rpx_runtime::{execution::render_request, form::RenderRequest};
use serde_json::{json, Value as J};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/r-model")
}
fn request(id: &str) -> RenderRequest {
    RenderRequest {
        report: std::fs::read(examples().join(format!("{id}.json"))).unwrap(),
        report_name: id.into(),
        params: ParamTable::EMPTY,
        datasets: ["sources.xlsx", "telecom.csv", "refunds.csv"]
            .into_iter()
            .map(|name| (name.into(), std::fs::read(examples().join(name)).unwrap()))
            .collect(),
    }
}
struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rpx-mixed-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn gallery_runs_all_models_and_rejects_changed_source_against_expectations() {
    let temp = Temporary::new();
    let input = temp.0.join("input");
    std::fs::create_dir(&input).unwrap();
    for file in std::fs::read_dir(examples()).unwrap() {
        let file = file.unwrap();
        if file.file_type().unwrap().is_file() {
            std::fs::copy(file.path(), input.join(file.file_name())).unwrap();
        }
    }
    let mut config = json!({});
    for name in ["operations", "organization"] {
        let url = format!(
            "sqlite://{}?mode=rwc",
            temp.0.join(format!("{name}.sqlite")).display()
        );
        let source = rpx_ds::SqlSource::connect(&url).await.unwrap();
        for sql in std::fs::read_to_string(input.join(format!("{name}.sql")))
            .unwrap()
            .split(';')
            .filter(|s| !s.trim().is_empty())
        {
            source.fetch_sql(sql).await.unwrap();
        }
        config[format!("{name}-db")] = json!(url);
    }
    let config_path = temp.0.join("connections.json");
    std::fs::write(&config_path, config.to_string()).unwrap();
    let output = temp.0.join("output");
    let catalog = rpx_runtime::gallery::generate(&input, &output, Some(&config_path))
        .await
        .unwrap();
    let reports = catalog["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 9);
    for report in reports {
        assert_eq!(report["status"], "succeeded");
        assert!(!report["run"]["checks"].as_array().unwrap().is_empty());
        let path = output
            .join(catalog["run_id"].as_str().unwrap())
            .join(report["id"].as_str().unwrap());
        for artifact in [
            "run.json",
            "result.json",
            "workbook.json",
            "report.xlsx",
            "report.csv",
            "template.json",
        ] {
            assert!(std::fs::metadata(path.join(artifact)).unwrap().len() > 0);
        }
        assert!(!path.join("connections.json").exists());
    }
    // 预期保持不变，只改输入；必须重新取数并且校验失败，不能复用旧计算结果。
    let refunds = input.join("refunds.csv");
    let text = std::fs::read_to_string(&refunds)
        .unwrap()
        .replacen("50.5", "60.5", 1);
    std::fs::write(refunds, text).unwrap();
    assert!(
        rpx_runtime::gallery::generate(&input, &output, Some(&config_path))
            .await
            .is_err()
    );
    let changed: J =
        serde_json::from_slice(&std::fs::read(output.join("catalog.json")).unwrap()).unwrap();
    assert_ne!(changed["run_id"], catalog["run_id"]);
    let mixed = changed["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "mixed-data-sources")
        .unwrap();
    assert_eq!(mixed["status"], "failed");
    assert!(mixed["error"].as_str().unwrap().contains("CSV 小数与引号"));
    assert!(
        output.join(catalog["run_id"].as_str().unwrap()).exists(),
        "保留前次运行产物"
    );
}

#[test]
fn adding_service_data_expands_columns_and_recalculates_ranks_and_top_three() {
    let mut req = request("dynamic-cell-calculation");
    let original: J = serde_json::from_str(&render_request(&req).unwrap()).unwrap();
    assert_eq!(original["sheets"]["rpx-sheet-1"]["printColumnCount"], 9);
    let mut definition: J = serde_json::from_slice(&req.report).unwrap();
    definition["datasets"][1]["file"] =
        json!({"file_name":"services.csv","encoding":"UTF-8","delimiter":","});
    req.report = serde_json::to_vec(&definition).unwrap();
    req.datasets.insert(
        "services.csv".into(),
        "service\n固定电话\n移动电话\n物联网\n5G专网\n"
            .as_bytes()
            .to_vec(),
    );
    let regions = [
        "朝阳",
        "崇文",
        "东城",
        "丰台",
        "海淀",
        "石景山",
        "西城",
        "宣武",
    ];
    let data = req.datasets.get_mut("telecom.csv").unwrap();
    for (i, region) in regions.iter().enumerate() {
        data.extend_from_slice(format!("{region},5G专网,{}\n", (i + 1) * 50).as_bytes());
    }
    let result: J = serde_json::from_str(&render_request(&req).unwrap()).unwrap();
    let sheet = &result["sheets"]["rpx-sheet-1"];
    assert_eq!(sheet["printColumnCount"], 11);
    for i in 0..8 {
        let row = &sheet["cellData"][(i + 4).to_string()];
        assert_eq!(row["9"]["v"], (i + 1) * 50);
        assert_eq!(row["10"]["v"], 8 - i);
    }
    assert_eq!(sheet["cellData"]["12"]["1"]["v"], 7200);
    assert_eq!(sheet["cellData"]["12"]["9"]["v"], 1050);
    assert_eq!(sheet["cellData"]["12"]["2"]["s"]["n"]["pattern"], "0.00%");
}

#[tokio::test]
async fn identical_queries_use_their_own_database_alias_and_unknown_alias_fails() {
    let registry = rpx_runtime::datasource::DataSourceRegistry::from_json(
        r#"{"operations":"sqlite::memory:","budget":"sqlite::memory:"}"#,
    )
    .await
    .unwrap();
    for (name, amount) in [("operations", 32), ("budget", 20)] {
        let source = registry.get(name).unwrap();
        source
            .fetch_sql("CREATE TABLE facts(amount INTEGER)")
            .await
            .unwrap();
        source
            .fetch_sql(&format!("INSERT INTO facts VALUES ({amount})"))
            .await
            .unwrap();
    }
    let mut definition = json!({"grid":{"rows":1,"cols":3},"datasets":[
        {"name":"actuals","sql":{"dsn":"operations","sql":"SELECT amount FROM facts"}},
        {"name":"plan","sql":{"dsn":"budget","sql":"SELECT amount FROM facts"}}
    ],"cells":[
        {"row":1,"col":1,"expr":"actuals.sum(amount)"},
        {"row":1,"col":2,"expr":"plan.sum(amount)"},
        {"row":1,"col":3,"expr":"A1-B1"}
    ]});
    let mut req = RenderRequest {
        report: serde_json::to_vec(&definition).unwrap(),
        report_name: "aliases".into(),
        params: ParamTable::EMPTY,
        datasets: BTreeMap::new(),
    };
    let result = rpx_runtime::execution::render_request_ds(&req, Some(&registry))
        .await
        .unwrap();
    let result: J = serde_json::from_str(&result).unwrap();
    assert_eq!(
        result["sheets"]["rpx-sheet-1"]["cellData"]["0"]["2"]["v"],
        12
    );
    definition["datasets"][1]["sql"]["dsn"] = json!("unknown");
    req.report = serde_json::to_vec(&definition).unwrap();
    let error = rpx_runtime::execution::render_request_ds(&req, Some(&registry))
        .await
        .unwrap_err();
    assert!(format!("{error:?}").contains("unknown"));
}

#[tokio::test]
async fn excel_and_gbk_csv_cross_multipart_and_validate_source_errors() {
    let book = std::fs::read(examples().join("sources.xlsx")).unwrap();
    let csv = b"region,refund\n\xbb\xaa\xb6\xab,50.5\n";
    let definition = json!({"grid":{"rows":1,"cols":3},"datasets":[
        {"name":"budget","file":{"file_name":"sources.xlsx","sheet_name":"预算"}},
        {"name":"refunds","file":{"file_name":"refunds.csv","encoding":"GBK","delimiter":","}}
    ],"cells":[
        {"row":1,"col":1,"expr":"budget.sum(budget)"},
        {"row":1,"col":2,"expr":"sum(refunds.select(refund,region=='华东'))"},
        {"row":1,"col":3,"expr":"A1-B1"}
    ]});
    let bytes = serde_json::to_vec(&definition).unwrap();
    let mut req = RenderRequest {
        report: bytes,
        report_name: "mixed".into(),
        params: ParamTable::EMPTY,
        datasets: BTreeMap::from([
            ("sources.xlsx".into(), book),
            ("refunds.csv".into(), csv.to_vec()),
        ]),
    };
    let result: J = serde_json::from_str(&render_request(&req).unwrap()).unwrap();
    assert_eq!(
        result["sheets"]["rpx-sheet-1"]["cellData"]["0"]["2"]["v"],
        4449.5
    );
    for (key, value, message) in [
        ("sheet_name", json!("不存在"), "不存在"),
        ("delimiter", json!("||"), "分隔符"),
    ] {
        let mut bad = definition.clone();
        bad["datasets"][0]["file"][key] = value;
        req.report = serde_json::to_vec(&bad).unwrap();
        assert!(format!("{:?}", render_request(&req).unwrap_err()).contains(message));
    }
    let mut duplicate = definition.clone();
    duplicate["datasets"][1]["name"] = json!("budget");
    req.report = serde_json::to_vec(&duplicate).unwrap();
    assert!(format!("{:?}", render_request(&req).unwrap_err()).contains("名称重复"));
    req.report = serde_json::to_vec(&definition).unwrap();
    req.datasets.remove("sources.xlsx");
    assert!(format!("{:?}", render_request(&req).unwrap_err()).contains("sources.xlsx"));
}
