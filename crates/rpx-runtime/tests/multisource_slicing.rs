//! 综合模板经 JSON / 保存后的 RPX、真实多源取数及分页快照运行。
use rpx_engine::params::ParamTable;
use rpx_runtime::{
    datasource::DataSourceRegistry, execution::render_request_ds, form::RenderRequest,
};
use serde_json::{json, Value as J};
use std::path::{Path, PathBuf};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/r-model")
}

async fn fixture() -> (RenderRequest, DataSourceRegistry) {
    let registry = DataSourceRegistry::from_json(
        r#"{"operations-db":"sqlite::memory:","organization-db":"sqlite::memory:"}"#,
    )
    .await
    .unwrap();
    for name in ["operations", "organization"] {
        let source = registry.get(&format!("{name}-db")).unwrap();
        for statement in std::fs::read_to_string(examples().join(format!("{name}.sql")))
            .unwrap()
            .split(';')
            .filter(|s| !s.trim().is_empty())
        {
            source.fetch_sql(statement).await.unwrap();
        }
    }
    let req = RenderRequest {
        report: std::fs::read(examples().join("multisource-slicing.json")).unwrap(),
        report_name: "multisource-slicing".into(),
        params: ParamTable::EMPTY,
        datasets: ["slicing.xlsx", "slicing-budget.csv"]
            .into_iter()
            .map(|name| (name.into(), std::fs::read(examples().join(name)).unwrap()))
            .collect(),
    };
    (req, registry)
}

async fn render(req: &RenderRequest, registry: &DataSourceRegistry) -> J {
    serde_json::from_str(&render_request_ds(req, Some(registry)).await.unwrap()).unwrap()
}

fn sheet(workbook: &J) -> &J {
    &workbook["sheets"]["rpx-sheet-1"]
}

fn number(sheet: &J, row: usize, col: usize) -> f64 {
    sheet["cellData"][row.to_string()][col.to_string()]["v"]
        .as_f64()
        .unwrap_or_else(|| panic!("缺少数值格 ({row},{col})"))
}

#[tokio::test]
async fn saved_rpx_preserves_roots_sources_slices_and_empty_order_totals() {
    let (mut req, registry) = fixture().await;
    let json_workbook = render(&req, &registry).await;
    let s = sheet(&json_workbook);
    assert_eq!(
        (s["printRowCount"].as_u64(), s["printColumnCount"].as_u64()),
        (Some(55), Some(14))
    );
    assert_eq!(
        s["freeze"],
        json!({"xSplit":2,"ySplit":4,"startRow":4,"startColumn":2})
    );
    assert_eq!(number(s, 9, 7), 3650.0);
    assert_eq!(number(s, 9, 8), 3350.0); // 阈值严格大于 300，排除相等边界。
    assert_eq!(number(s, 14, 7), 0.0); // Excel 员工 4 没有业务，仍保留预算。
    assert_eq!(number(s, 14, 12), 100.0);
    assert_eq!(number(s, 53, 8), 0.0); // 订单 109 空明细不能沿用上一单 800。
    assert_eq!(number(s, 53, 13), 37.0);
    assert_eq!(number(s, 54, 11), 187.0); // 订单级运费独立于明细粒度。
    assert_eq!(number(s, 54, 13), 3837.0);

    let rd = rpx_runtime::reportdef::open_report(&req.report).unwrap();
    req.report = rpx_core::to_bytes(&rd).unwrap();
    let saved = render(&req, &registry).await;
    assert_eq!(saved, json_workbook, "保存模板后走同一执行路径且逐格一致");
}

#[tokio::test]
async fn changing_csv_and_adding_database_year_city_and_order_recomputes_dimensions() {
    let (mut req, registry) = fixture().await;
    let before = render(&req, &registry).await;
    let csv = req.datasets.get_mut("slicing-budget.csv").unwrap();
    *csv = String::from_utf8(csv.clone())
        .unwrap()
        .replace("2025,3,900", "2025,3,990")
        .into_bytes();
    let changed_budget = render(&req, &registry).await;
    assert_eq!(number(sheet(&changed_budget), 8, 12), 990.0);
    assert_eq!(number(sheet(&changed_budget), 9, 12), 3790.0);
    assert_eq!(number(sheet(&before), 8, 12), 900.0);
    assert_eq!(number(sheet(&changed_budget), 54, 13), 3837.0);

    let source = registry.get("operations-db").unwrap();
    source
        .fetch_sql(
            "INSERT INTO slice_orders VALUES (110,'2026-01-01','West','W4',4,'Juniper',2,41)",
        )
        .await
        .unwrap();
    source
        .fetch_sql("INSERT INTO slice_lines VALUES (10,110,10,1,1000)")
        .await
        .unwrap();
    let changed = render(&req, &registry).await;
    let s = sheet(&changed);
    assert_eq!(s["printRowCount"], 60);
    assert_eq!(s["printColumnCount"], 15);
    assert_eq!(number(s, 9, 0), 2026.0);
    assert_eq!(number(s, 9, 1), 1.0);
    assert_eq!(number(s, 9, 7), 1000.0); // 新城市增加交叉列。
    assert_eq!(number(s, 9, 13), 0.0); // 新月份没有 CSV 预算。
    assert_eq!(number(s, 10, 8), 4650.0);
    assert_eq!(number(s, 59, 12), 228.0);
    assert_eq!(number(s, 59, 14), 4878.0);
    assert_eq!(s["freeze"], sheet(&before)["freeze"]);
}

#[tokio::test]
async fn horizontal_and_vertical_pages_repeat_headers_with_native_freeze() {
    let (mut req, registry) = fixture().await;
    let mut definition: J = serde_json::from_slice(&req.report).unwrap();
    definition["printing"]["horizontal_split"] = json!(true);
    definition["printing"]["vertical_split"] = json!(true);
    definition["print_setup"]["paper_width"] = json!(210);
    definition["print_setup"]["paper_height"] = json!(148);
    req.report = serde_json::to_vec(&definition).unwrap();
    let workbook = render(&req, &registry).await;
    let sheets = workbook["sheets"].as_object().unwrap();
    assert!(sheets.len() > 4, "综合宽长表必须横纵分页");
    for s in sheets.values() {
        assert_eq!(s["freeze"]["xSplit"], 2);
        assert_eq!(s["freeze"]["ySplit"], 4);
        assert_eq!(s["cellData"]["2"]["0"]["v"], "年");
        assert_eq!(s["cellData"]["2"]["1"]["v"], "月 / 员工编号");
    }
}
