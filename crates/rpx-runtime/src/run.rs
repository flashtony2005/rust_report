use crate::{
    datasource::DataSourceRegistry,
    definition,
    error::{Error, Result},
    execution, files,
    form::RenderRequest,
    parameters, result, snapshot,
};
use serde_json::{json, Value as J};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct Options {
    pub data_dir: Option<PathBuf>,
    pub params: Option<J>,
    pub datasources: Option<PathBuf>,
    pub exports: Vec<String>,
}
pub struct Source {
    pub template: J,
    pub name: String,
    pub data_dir: PathBuf,
    pub params: J,
    pub report_id: Option<String>,
    pub revision: String,
    pub files: Vec<(String, Vec<u8>)>,
}
pub fn capture(path: &Path, options: &Options) -> Result<Source> {
    let path = files::absolute(path)?;
    let bytes = fs::read(&path)?;
    let template = serde_json::from_slice(&bytes)?;
    Ok(Source {
        template,
        name: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        data_dir: options
            .data_dir
            .clone()
            .unwrap_or_else(|| path.parent().unwrap().to_path_buf()),
        params: options.params.clone().unwrap_or_else(|| json!({})),
        report_id: None,
        revision: files::hash(&bytes),
        files: vec![(path.file_name().unwrap().to_string_lossy().into(), bytes)],
    })
}
pub fn start(source: &Source, out: &Path) -> Result<J> {
    fs::create_dir(out)?;
    let record = json!({"version":1,"run_id":out.file_name().unwrap_or_default().to_string_lossy(),
        "status":"running","started_at":timestamp(),"finished_at":null,"engine_version":env!("CARGO_PKG_VERSION"),
        "report_id":source.report_id,"revision":source.revision,"parameters":source.params,"inputs":[],"artifacts":{},"diagnostics":[],
        "source_files":source.files.iter().map(|(p,_)|format!("source/{p}")).collect::<Vec<_>>()});
    files::atomic_json(&out.join("run.json"), &record)?;
    Ok(record)
}
pub async fn execute(source: Source, options: &Options, out: &Path, mut record: J) -> Result<J> {
    match generate(&source, options, out, &mut record).await {
        Ok(()) => {
            record["status"] = json!("succeeded");
            record["finished_at"] = json!(timestamp());
            files::atomic_json(&out.join("run.json"), &record)?;
            Ok(
                json!({"run":record,"out_dir":files::absolute(out)?,"result":files::absolute(&out.join("artifacts/result.json"))?,"workbook":files::absolute(&out.join("artifacts/workbook.json"))?}),
            )
        }
        Err(mut e) => {
            record["status"] = json!("failed");
            record["finished_at"] = json!(timestamp());
            record["artifacts"] = json!({});
            record["diagnostics"] = json!([e.diagnostic]);
            files::atomic_json(&out.join("run.json"), &record)?;
            let _ = fs::remove_dir_all(out.join(".artifacts"));
            e.diagnostic["run_file"] = json!(files::absolute(&out.join("run.json"))?);
            Err(e)
        }
    }
}
async fn generate(source: &Source, options: &Options, out: &Path, record: &mut J) -> Result<()> {
    for (name, bytes) in &source.files {
        let p = files::relative(&out.join("source"), name)?;
        fs::create_dir_all(p.parent().unwrap())?;
        files::create(&p, bytes)?;
    }
    fs::create_dir(out.join("inputs"))?;
    files::create(
        &out.join("inputs/params.json"),
        &files::json_bytes(&source.params)?,
    )?;
    definition::validate(&source.template, Some(&source.params))?;
    let (rd, macros) = definition::parse(&source.template)?;
    let mut datasets = BTreeMap::new();
    let mut inputs = vec![
        json!({"kind":"parameters","sha256":files::hash(&files::json_bytes(&source.params)?)}),
    ];
    for (i, d) in rd.datasets.iter().enumerate() {
        if let Some(f) = &d.file {
            let name = f.file_name.as_deref().unwrap_or("");
            let path = source.data_dir.join(name);
            let bytes = fs::read(&path).map_err(|e| {
                Error::new(
                    4,
                    "DATA_MISSING",
                    "data",
                    format!("{}: {e}", path.display()),
                )
                .at(format!("/datasets/{i}/file/file_name"))
            })?;
            inputs.push(json!({"kind":"file","dataset":f.name,"path":files::absolute(&path)?,"sha256":files::hash(&bytes)}));
            datasets.insert(name.into(), bytes);
        } else if let Some(s) = &d.sql {
            let sql = rpx_engine::substitute_macros(s.sql.as_deref().unwrap_or(""), &macros)
                .map_err(|e| Error::new(4, "SQL_MACRO", "data", e))?;
            inputs.push(json!({"kind":"sql","dataset":s.name,"dsn":s.data_source_name,"query_sha256":files::hash(sql.as_bytes())}));
        }
    }
    record["inputs"] = json!(inputs);
    let registry = match &options.datasources {
        Some(p) => Some(DataSourceRegistry::from_file(p).await.map_err(|_| {
            Error::new(
                4,
                "DATASOURCE_CONFIG",
                "data",
                "数据源配置或连接失败，请检查配置与连接",
            )
        })?),
        None => None,
    };
    let req = RenderRequest {
        report: files::json_bytes(&source.template)?,
        report_name: source.name.clone(),
        datasets,
        params: parameters::parse(&source.params)?,
    };
    let sql = execution::prefetch_sql(&rd, registry.as_ref(), &macros)
        .await
        .map_err(|e| Error::new(4, "DATA_FETCH", "data", e))?;
    let datasets =
        crate::data::load(&req, &rd, &sql).map_err(|e| Error::new(4, "DATA_FETCH", "data", e))?;
    let grid = execution::calculate_datasets(&rd, &datasets, &req.params, &macros)
        .map_err(|e| Error::new(5, "CALCULATION_FAILED", "calculate", e))?;
    let staged = out.join(".artifacts");
    fs::create_dir(&staged)?;
    let workbook = snapshot::build(&rd, &grid, &source.name)
        .map_err(|e| Error::new(6, "SNAPSHOT_FAILED", "snapshot", e))?;
    files::create(
        &staged.join("result.json"),
        &files::json_bytes(&result::build(&grid, &source.template))?,
    )?;
    files::create(&staged.join("workbook.json"), workbook.as_bytes())?;
    let mut artifacts =
        json!({"result.json":"artifacts/result.json","workbook.json":"artifacts/workbook.json"});
    let mut export_warnings = Vec::new();
    for kind in &options.exports {
        let (name, result): (&str, std::result::Result<Vec<u8>, String>) = match kind.as_str() {
            "csv" => ("report.csv", Ok(rpx_export::to_csv(&grid).into_bytes())),
            "xlsx" => ("report.xlsx", Ok(rpx_export::to_xlsx(&grid))),
            "print-xlsx" => (
                "report-print.xlsx",
                rpx_export::to_print_xlsx(&grid, &grid.print_setup),
            ),
            _ => {
                return Err(Error::new(
                    2,
                    "USAGE",
                    "arguments",
                    "导出格式可选 csv / xlsx / print-xlsx",
                ))
            }
        };
        match result {
            Ok(bytes) => {
                if !artifacts.as_object().unwrap().contains_key(name) {
                    files::create(&staged.join(name), &bytes)?;
                    artifacts[name] = json!(format!("artifacts/{name}"));
                }
            }
            Err(message) => export_warnings.push(json!({
                "severity": "warning",
                "code": "EXPORT_FAILED",
                "stage": "export",
                "format": kind,
                "message": message,
            })),
        }
    }
    fs::rename(staged, out.join("artifacts"))?;
    record["artifacts"] = artifacts;
    record["diagnostics"] = json!(export_warnings);
    record["shape"] = json!({"rows":grid.rows,"cols":grid.cols,"pages":grid.page_count});
    Ok(())
}
pub fn timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("UTC timestamp")
}
pub async fn inspect_dataset(
    source: Source,
    options: &Options,
    name: &str,
    limit: usize,
) -> Result<J> {
    let (mut rd, macros) = definition::parse(&source.template)?;
    rd.datasets.retain(|d| {
        d.file
            .as_ref()
            .and_then(|f| f.name.as_deref())
            .or_else(|| d.sql.as_ref().and_then(|s| s.name.as_deref()))
            == Some(name)
    });
    if rd.datasets.is_empty() {
        return Err(Error::new(4, "DATASET_UNKNOWN", "data", name));
    }
    let mut datasets = BTreeMap::new();
    for d in &rd.datasets {
        if let Some(f) = &d.file {
            let n = f.file_name.as_deref().unwrap();
            datasets.insert(
                n.into(),
                fs::read(source.data_dir.join(n))
                    .map_err(|e| Error::new(4, "DATA_MISSING", "data", e))?,
            );
        }
    }
    let registry = match &options.datasources {
        Some(p) => Some(
            DataSourceRegistry::from_file(p)
                .await
                .map_err(|_| Error::new(4, "DATASOURCE_CONFIG", "data", "数据源配置或连接失败"))?,
        ),
        None => None,
    };
    let sql = execution::prefetch_sql(&rd, registry.as_ref(), &macros)
        .await
        .map_err(|e| Error::new(4, "DATA_FETCH", "data", e))?;
    let req = RenderRequest {
        report: vec![],
        report_name: source.name,
        datasets,
        params: parameters::parse(&source.params)?,
    };
    let rows =
        crate::data::load(&req, &rd, &sql).map_err(|e| Error::new(4, "DATA_FETCH", "data", e))?;
    let ds = &rows[0];
    let items: Vec<_> = ds
        .rows
        .iter()
        .map(|r| json!(r.iter().map(result::typed).collect::<Vec<_>>()))
        .collect();
    Ok(
        json!({"name":name,"columns":ds.cols,"rows":ds.rows.len(),"sample":files::page(&items,limit,0)?}),
    )
}
