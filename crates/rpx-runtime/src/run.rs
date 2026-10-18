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
                json!({"run":record,"out_dir":files::absolute(out)?,"result":files::absolute(&out.join("artifacts/result.json"))?,"workbook":files::absolute(&out.join("artifacts/workbook.json"))?,"layout":record["layout"].clone()}),
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
                .hint(format!(
                    "file.file_name 是相对路径，基准目录当前为 {}；用 --data-dir 指向数据集目录后重跑",
                    source.data_dir.display()
                ))
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
            .hint(format!(
                "配置文件 {} 需为 {{name: url}} 或 {{name: {{url,user,pass}}}}；只做形状检查可用 rpx doctor --datasources",
                p.display()
            ))
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
        .map_err(|e| {
            let hint = match &e {
                execution::RenderError::NoDataSource { dsn } => Some(format!(
                    "模板里有 SQL 数据集指向 dsn「{dsn}」，但本次没有数据源注册表；用 --datasources 传入，或先把 SQL 结果落成 CSV"
                )),
                execution::RenderError::UnknownDataSource { dsn, available } => Some(format!(
                    "dsn「{dsn}」未在注册表里；已登记：{}",
                    available.join(", ")
                )),
                _ => None,
            };
            Error::new(4, "DATA_FETCH", "data", e).hint_if_some(hint)
        })?;
    let datasets = crate::data::load(&req, &rd, &sql).map_err(|e| {
        let hint = data_hint(&e);
        Error::new(4, "DATA_FETCH", "data", e).hint_if_some(hint)
    })?;
    let grid =
        execution::calculate_datasets(&rd, &datasets, &req.params, &macros).map_err(|e| {
            let hint = match &e {
                execution::RenderError::Engine(rpx_engine::EngineError::UnknownCell { id }) => {
                    unknown_cell_hint(id, &datasets)
                }
                _ => data_hint(&e),
            };
            Error::new(5, "CALCULATION_FAILED", "calculate", e).hint_if_some(hint)
        })?;
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
    record["layout"] = snapshot::layout_json(&grid);
    Ok(())
}

/// 数据集类错误的修复提示。诊断说"哪一步炸了"，hint 说"下一步查什么"。
fn data_hint(e: &execution::RenderError) -> Option<String> {
    match e {
        execution::RenderError::Data(message) => Some(format!(
            "数据集解析失败（{message}）。按这个顺序查：① file.delimiter 是否显式写成 \",\"；② encoding 是否与实际一致；③ 首行是不是表头。\n用 `rpx dataset inspect FILE --name NAME` 可以看到引擎真正解析出的列名"
        )),
        execution::RenderError::MissingFile { file, dataset } => Some(format!(
            "模板里数据集「{dataset}」引用文件 {file}，但本次输入没有它；用 --data-dir 指向该文件所在目录"
        )),
        execution::RenderError::Engine(rpx_engine::EngineError::UnknownParam { name }) => Some(
            format!("参数「{name}」没有绑定；用 --params-file 传入 {{\"{name}\": ...}}，或在模板 params 里声明默认值"),
        ),
        _ => None,
    }
}

/// `UnknownCell` 的提示：这个错误九成不是"格号写错"，而是"列名根本没解析出来"。
///
/// 关键判别：某数据集的 `cols` 只有一项且本身含分隔符 ⇒ 文件没被切分，
/// 此时列名就是整行文本，表达式里任何 `ds.field` 都会报 UnknownCell。
fn unknown_cell_hint(id: &str, datasets: &[rpx_engine::expr::ds::DataSet]) -> Option<String> {
    // 裸格号（`$B$2`）没有点号 ⇒ 确实是格号问题，不该编提示。
    let (ds_name, field) = id.split_once('.')?;
    let Some(ds) = datasets.iter().find(|d| d.name == ds_name) else {
        let names: Vec<&str> = datasets.iter().map(|d| d.name.as_str()).collect();
        return Some(format!(
            "已加载的数据集是 [{}]；表达式里的「{ds_name}」不在其中（大小写敏感）",
            names.join(", ")
        ));
    };
    if let Some(bad) = ds.cols.iter().find(|c| {
        c.len() > 1 && (c.contains(',') || c.contains(';') || c.contains('\t') || c.contains('|'))
    }) {
        let delimiter = if bad.contains('\t') {
            "\\t".to_string()
        } else if bad.contains(',') {
            ",".to_string()
        } else if bad.contains(';') {
            ";".to_string()
        } else {
            "|".to_string()
        };
        return Some(format!(
            "数据集「{}」只解析出 1 列「{}」，说明文件没被切分（缺 delimiter）。\n修法：给该数据集的 file 加 \"delimiter\": \"{}\"；确认列名用 `rpx dataset inspect FILE --name {}`",
            ds.name, bad, delimiter, ds.name
        ));
    }
    let preview: Vec<&str> = ds.cols.iter().take(12).map(String::as_str).collect();
    Some(format!(
        "数据集「{}」的列是 [{}]{}；`{field}` 不在其中（列名大小写敏感，中文列名不要加空格）",
        ds.name,
        preview.join(", "),
        if ds.cols.len() > preview.len() {
            " …"
        } else {
            ""
        }
    ))
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
