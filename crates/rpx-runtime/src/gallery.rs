//! Agent 离线执行示例目录：一次计算同时保存校验结果、Univer 快照与导出文件。
use crate::{datasource::DataSourceRegistry, execution, form::RenderRequest, reportdef, snapshot};
use rpx_core::value::Value;
use rpx_engine::{arena::CalcGrid, params::ParamTable};
use serde_json::{json, Value as J};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub async fn generate(
    source: &Path,
    output: &Path,
    datasources: Option<&Path>,
) -> Result<J, String> {
    let input: J = serde_json::from_slice(
        &std::fs::read(source.join("catalog.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let reports = input["reports"]
        .as_array()
        .ok_or("catalog.reports 必须是数组")?;
    let registry = match datasources {
        Some(path) => Some(
            DataSourceRegistry::from_file(path)
                .await
                .map_err(|e| e.to_string())?,
        ),
        None => None,
    };
    let run_id = format!(
        "run-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let root = output.join(&run_id);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let mut generated = Vec::new();
    let mut failed = 0;
    for report in reports {
        let id = field(report, "id")?;
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(format!("无效报表 ID：{id}"));
        }
        let dest = root.join(id);
        std::fs::create_dir(&dest).map_err(|e| e.to_string())?;
        let mut record = report.clone();
        record["base"] = json!(format!("/report-gallery/{run_id}/{id}"));
        record["run_id"] = json!(run_id);
        match generate_one(source, &dest, report, registry.as_ref()).await {
            Ok(summary) => {
                record["status"] = json!("succeeded");
                record["run"] = summary;
            }
            Err(error) => {
                failed += 1;
                record["status"] = json!("failed");
                record["error"] = json!(error);
            }
        }
        write_json(&dest.join("run.json"), &record)?;
        generated.push(record);
    }
    let catalog = json!({"version":1,"source":input["source"],"run_id":run_id,"reports":generated});
    // 最后发布索引；浏览器始终通过索引读取某一次完整构建的产物。
    write_json(&output.join("catalog.pending.json"), &catalog)?;
    std::fs::rename(
        output.join("catalog.pending.json"),
        output.join("catalog.json"),
    )
    .map_err(|e| e.to_string())?;
    if failed > 0 {
        return Err(format!(
            "{failed} 份报表生成失败，详情见 {}",
            output.join("catalog.json").display()
        ));
    }
    Ok(catalog)
}

async fn generate_one(
    source: &Path,
    dest: &Path,
    report: &J,
    registry: Option<&DataSourceRegistry>,
) -> Result<J, String> {
    let bytes =
        std::fs::read(source.join(field(report, "template")?)).map_err(|e| e.to_string())?;
    let (rd, macros) = reportdef::open_report_full(&bytes).map_err(|e| e.to_string())?;
    let mut files = BTreeMap::new();
    let mut inputs = Vec::new();
    std::fs::write(dest.join("template.json"), &bytes).map_err(|e| e.to_string())?;
    for entry in &rd.datasets {
        if let Some(file) = &entry.file {
            let filename = file
                .file_name
                .as_deref()
                .ok_or("文件数据集没有 file_name")?;
            // 示例文件名为包内平面文件；不把任意路径复制到可公开的展示目录。
            if Path::new(filename).file_name().and_then(|s| s.to_str()) != Some(filename) {
                return Err(format!("示例文件名必须位于包根目录：{filename}"));
            }
            let data =
                std::fs::read(source.join(filename)).map_err(|e| format!("{filename}: {e}"))?;
            std::fs::write(dest.join(filename), &data).map_err(|e| e.to_string())?;
            files.insert(filename.to_string(), data);
            inputs.push(json!({"name":file.name,"kind":if rpx_ds::file_source::is_excel(filename){"Excel"}else{"CSV"},"file":filename,"sheet":file.sheet_name}));
        } else if let Some(sql) = &entry.sql {
            inputs.push(json!({"name":sql.name,"kind":"SQL","dsn":sql.data_source_name}));
        }
    }
    let req = RenderRequest {
        report: bytes,
        report_name: field(report, "title")?.into(),
        datasets: files,
        params: ParamTable::EMPTY,
    };
    let rows = execution::prefetch_sql(&rd, registry, &macros)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let grid =
        execution::calculate_grid(&req, &rd, &rows, &macros).map_err(|e| format!("{e:?}"))?;
    let checks = check(&grid, report)?;
    let workbook = snapshot::build(&rd, &grid, &req.report_name)?;
    let cells: Vec<_> = grid.cells.iter().filter(|c| c.is_emitted()).map(|c| json!({
        "row":c.row,"col":c.col,"origin":[c.origin.0,c.origin.1],"type":c.value.type_name(),"value":value(&c.value),
        "display_override":c.disp_value,"left_head_origin":c.left_head,"top_head_origin":c.top_head
    })).collect();
    write_json(
        &dest.join("result.json"),
        &json!({"version":1,"rows":grid.rows,"cols":grid.cols,"cells":cells}),
    )?;
    std::fs::write(dest.join("workbook.json"), workbook).map_err(|e| e.to_string())?;
    std::fs::write(dest.join("report.csv"), rpx_export::to_csv(&grid))
        .map_err(|e| e.to_string())?;
    std::fs::write(dest.join("report.xlsx"), rpx_export::to_xlsx(&grid))
        .map_err(|e| e.to_string())?;
    Ok(
        json!({"template_rows":rd.rows-1,"template_cols":rd.cols-1,"rows":grid.rows,"cols":grid.cols,"pages":grid.page_count,"inputs":inputs,"checks":checks}),
    )
}

fn field<'a>(j: &'a J, key: &str) -> Result<&'a str, String> {
    j[key]
        .as_str()
        .ok_or_else(|| format!("缺少字符串字段 {key}"))
}

fn write_json(path: &Path, value: &J) -> Result<(), String> {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn value(v: &Value) -> J {
    match v {
        Value::Null => J::Null,
        Value::Bool(n) => json!(n),
        Value::Byte(n) => json!(n),
        Value::Short(n) => json!(n),
        Value::Int(n) => json!(n),
        Value::Long(n) => json!(n.to_string()),
        Value::Float(n) => json!(n),
        Value::Double(n) => json!(n),
        Value::Decimal(s) | Value::Text(s) => json!(s),
        Value::List(v) => J::Array(v.iter().map(value).collect()),
    }
}

fn check(grid: &CalcGrid, report: &J) -> Result<Vec<J>, String> {
    let cases = report["checks"]
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or("每个示例必须声明独立预期检查")?;
    let mut results = Vec::new();
    for case in cases {
        let row = case["origin"][0].as_i64().ok_or("check.origin 需要行列")? as i32;
        let col = case["origin"][1].as_i64().ok_or("check.origin 需要行列")? as i32;
        let mut cells: Vec<_> = grid
            .cells
            .iter()
            .filter(|c| c.origin == (row, col) && c.is_emitted())
            .collect();
        cells.sort_by_key(|c| (c.row, c.col));
        let actual: Vec<J> = cells.iter().map(|c| value(&c.value)).collect();
        let expected = case["values"].as_array().ok_or("check.values 需要数组")?;
        let matches = actual.len() == expected.len()
            && actual.iter().zip(expected).all(|(a, b)| {
                let number = |v: &J| {
                    v.as_f64()
                        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                };
                match (number(a), number(b)) {
                    (Some(a), Some(b)) => (a - b).abs() < 1e-8,
                    _ => a == b,
                }
            });
        if !matches {
            return Err(format!(
                "校验失败 {}：期望 {expected:?}，实际 {actual:?}",
                case["label"]
            ));
        }
        results.push(
            json!({"label":case["label"],"origin":case["origin"],"actual":actual,"passed":true}),
        );
    }
    Ok(results)
}
