use crate::{
    error::{Error, Result},
    parameters, reportdef,
};
use rpx_core::ReportDefine;
use rpx_engine::MacroTable;
use serde_json::{json, Value as J};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

mod dependencies;

pub use rpx_format::definition::{schema, strict};

pub fn pointer(path: &str) -> String {
    path.trim_start_matches('$')
        .replace('[', ".")
        .replace(']', "")
        .split('.')
        .filter(|s| !s.is_empty())
        .map(|s| format!("/{}", s.replace('~', "~0").replace('/', "~1")))
        .collect()
}

pub fn parse(j: &J) -> Result<(ReportDefine, MacroTable)> {
    reportdef::open_report_full(&serde_json::to_vec(j)?).map_err(|e| {
        let path = match &e {
            reportdef::SourceError::Def(reportdef::DefError::Schema { path, .. }) => pointer(path),
            _ => String::new(),
        };
        Error::new(3, "TEMPLATE_INVALID", "definition", e).at(path)
    })
}
pub fn validate(j: &J, params: Option<&J>) -> Result<J> {
    let (rd, macros) = parse(j)?;
    let mut positions = HashSet::new();
    for (i, c) in j["cells"].as_array().into_iter().flatten().enumerate() {
        if !positions.insert((c["row"].clone().to_string(), c["col"].clone().to_string())) {
            return Err(Error::new(3, "CELL_DUPLICATE", "definition", "单元格重复")
                .at(format!("/cells/{i}")));
        }
    }
    let values = params.cloned().unwrap_or_else(|| json!({}));
    let table = parameters::parse(&values)?;
    dependencies::check(j, &table, &macros)?;
    let mut deferred = vec!["data_loading", "calculation", "pagination", "export"];
    let mut checked = vec!["schema", "declared_heads", "unconditional_scalar_cycles"];
    // Check each expression independently so a missing parameter cannot hide a later syntax error.
    let mut binding_missing = false;
    for (i, c) in j["cells"].as_array().into_iter().flatten().enumerate() {
        if c.get("literal").is_some()
            && rd
                .cell_at(
                    c["row"].as_u64().unwrap() as usize,
                    c["col"].as_u64().unwrap() as usize,
                )
                .is_some_and(|cell| rpx_engine::literal_value(&cell.value).is_none())
        {
            return Err(Error::new(
                3,
                "LITERAL_UNSUPPORTED",
                "prepare",
                "引擎尚不支持该字面量类型的计算",
            )
            .at(format!("/cells/{i}/literal")));
        }
        if let Some(expr) = c["expr"].as_str() {
            let expr = rpx_engine::substitute_macros(expr, &macros).map_err(|e| {
                Error::new(3, "FORMULA_INVALID", "prepare", e).at(format!("/cells/{i}/expr"))
            })?;
            if let Err(e) = rpx_engine::parse_with(&expr, &table) {
                if matches!(e, rpx_engine::EngineError::UnknownParam { .. }) && params.is_none() {
                    binding_missing = true;
                } else {
                    return Err(Error::new(
                        if matches!(e, rpx_engine::EngineError::UnknownParam { .. }) {
                            4
                        } else {
                            3
                        },
                        "FORMULA_INVALID",
                        "prepare",
                        e,
                    )
                    .at(format!("/cells/{i}/expr")));
                }
            }
        }
    }
    if binding_missing {
        deferred.extend(["parameter_binding", "inferred_heads"]);
    } else {
        rpx_engine::prepare_calculate_with(&rd, &table, &macros)
            .map_err(|e| Error::new(3, "PREPARE_FAILED", "prepare", e))?;
        checked.extend(["formula_preparation", "parameter_binding", "inferred_heads"]);
    }
    Ok(
        json!({"valid":true,"checked":checked,"deferred":deferred,"grid":{"rows":rd.rows-1,"cols":rd.cols-1}}),
    )
}
/// 校验 + 可选数据探针。
///
/// `probe` 非空时，除了结构校验，还会**真的去读**文件数据集并核对列名。
/// 这是把 `UnknownCell` 这类"跑到 run 才炸"的问题提前到 validate 的唯一办法。
/// 不传时行为与 `validate` 完全一致（保持离线可用）。
pub fn validate_with(template: &J, params: Option<&J>, probe: Option<&Path>) -> Result<J> {
    let mut out = validate(template, params)?;
    if let Some(dir) = probe {
        out["probe"] = probe_data(template, params, dir)?;
    }
    Ok(out)
}

/// 读文件数据集并核对"表达式引用的列"是否存在。
///
/// 只探文件数据集：SQL 数据集需要连接，`--datasources` 不在这一层处理，
/// 会以 `skipped` 明示，避免让人误以为已经验证过。
pub fn probe_data(template: &J, params: Option<&J>, data_dir: &Path) -> Result<J> {
    let (mut rd, _macros) = parse(template)?;
    let values = params.cloned().unwrap_or_else(|| json!({}));
    let mut files = BTreeMap::new();
    let mut skipped = Vec::new();
    for (index, entry) in rd.datasets.iter().enumerate() {
        if let Some(file) = &entry.file {
            let file_name = file.file_name.as_deref().unwrap_or("");
            let name = file.name.as_deref().unwrap_or(file_name);
            let path = data_dir.join(file_name);
            let bytes = std::fs::read(&path).map_err(|e| {
                Error::new(
                    4,
                    "DATA_MISSING",
                    "probe",
                    format!("数据集 {name}：{}: {e}", path.display()),
                )
                .at(format!("/datasets/{index}/file/file_name"))
                .hint(format!(
                    "探针的基准目录是 {}；用 --data-dir 指向数据集目录，或修正 file.file_name",
                    data_dir.display()
                ))
            })?;
            files.insert(file_name.to_string(), bytes);
        } else if let Some(sql) = &entry.sql {
            skipped.push(json!({
                "dataset": sql.name,
                "reason": "sql 数据集需要连接，探针不覆盖；用 rpx dataset inspect 或 run --datasources 验证",
            }));
        }
    }
    rd.datasets.retain(|d| d.file.is_some());
    let request = crate::form::RenderRequest {
        report: serde_json::to_vec(template)?,
        report_name: "probe".into(),
        datasets: files,
        params: parameters::parse(&values)?,
    };
    let loaded = crate::data::load(&request, &rd, &HashMap::new()).map_err(|e| {
        Error::new(4, "DATA_FETCH", "probe", e).hint(
            "探针能读文件但解析失败时，优先检查：① file.delimiter 是否显式写 \",\"；② encoding；③ 首行是不是表头",
        )
    })?;

    // 列名形状：单列且本身含分隔符 ⇒ 文件根本没被切分。这是 UnknownCell 的真凶。
    for ds in &loaded {
        if ds.cols.len() != 1 {
            continue;
        }
        let only = &ds.cols[0];
        let Some(sep) = [',', ';', '\t', '|']
            .into_iter()
            .find(|c| only.contains(*c))
        else {
            continue;
        };
        let pointer = template["datasets"]
            .as_array()
            .into_iter()
            .flatten()
            .position(|d| {
                d["file"]["name"].as_str() == Some(ds.name.as_str())
                    || d["name"].as_str() == Some(ds.name.as_str())
            })
            .map(|i| format!("/datasets/{i}/file/delimiter"));
        let literal = if sep == '\t' {
            "\\t".to_string()
        } else {
            sep.to_string()
        };
        let mut error = Error::new(
            4,
            "DATA_SHAPE",
            "probe",
            format!(
                "数据集 {} 只解析出 1 列「{only}」，文件没有被切分",
                ds.name
            ),
        )
        .hint(format!(
            "给该数据集的 file 加 \"delimiter\": \"{literal}\" 后重跑；确认列名用 `rpx dataset inspect`"
        ));
        if let Some(pointer) = pointer {
            error = error.at(pointer);
        }
        return Err(error);
    }

    // 表达式引用的列是否存在。列名错在 run 阶段表现为 UnknownCell，这里提前提示。
    let columns: BTreeMap<&str, &Vec<String>> =
        loaded.iter().map(|d| (d.name.as_str(), &d.cols)).collect();
    let mut warnings = Vec::new();
    for (index, cell) in template["cells"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let Some(expr) = cell["expr"].as_str() else {
            continue;
        };
        let address = address(
            cell["row"].as_u64().unwrap_or(0),
            cell["col"].as_u64().unwrap_or(0),
        );
        for (dataset, field) in column_refs(expr) {
            let Some(cols) = columns.get(dataset.as_str()) else {
                continue;
            };
            if cols.iter().any(|c| c == &field) {
                continue;
            }
            let preview: Vec<&str> = cols.iter().take(12).map(String::as_str).collect();
            warnings.push(json!({
                "code": "DATA_COLUMN",
                "level": "warning",
                "stage": "probe",
                "message": format!("{address} 引用了 {dataset}.{field}，但该数据集的列是 [{}]", preview.join(", ")),
                "hint": "列名大小写敏感、前后不能有空格；中文列名要按 CSV 表头原样写",
                "json_pointer": format!("/cells/{index}/expr"),
            }));
        }
    }

    let datasets: Vec<J> = loaded
        .iter()
        .map(|d| {
            json!({
                "name": d.name,
                "rows": d.rows.len(),
                "columns": d.cols,
                "columns_ok": !d.cols.is_empty(),
            })
        })
        .collect();
    let rows_total: usize = loaded.iter().map(|d| d.rows.len()).sum();
    Ok(json!({
        "probed": true,
        "data_dir": data_dir.display().to_string(),
        "datasets": datasets,
        "rows_total": rows_total,
        "skipped": skipped,
        "warnings": warnings,
    }))
}

/// 从表达式里抽 `数据集.函数(第一个裸标识符参数)`。只认裸标识符，
/// 因此 `select(base+bonus)` 这类复合表达式不会被误判成列名。
fn column_refs(expr: &str) -> Vec<(String, String)> {
    let bytes = expr.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let is_ident_start = bytes[i].is_ascii_alphabetic() || bytes[i] == b'_';
        if !is_ident_start {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        let ident = &expr[start..i];
        if bytes.get(i) != Some(&b'.') {
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
            j += 1;
        }
        if j == i + 1 || bytes.get(j) != Some(&b'(') {
            continue;
        }
        if let Some(argument) = first_bare_argument(&expr[j + 1..]) {
            out.push((ident.to_string(), argument));
        }
        i = j + 1;
    }
    out
}

fn first_bare_argument(rest: &str) -> Option<String> {
    let mut depth = 0i32;
    let mut quote = None::<char>;
    let mut end = rest.len();
    for (offset, ch) in rest.char_indices() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"') | (None, '\'') => quote = Some(ch),
            (None, '(') => depth += 1,
            (None, ')') if depth == 0 => {
                end = offset;
                break;
            }
            (None, ')') => depth -= 1,
            (None, ',') if depth == 0 => {
                end = offset;
                break;
            }
            _ => {}
        }
    }
    let argument = rest[..end].trim();
    let mut chars = argument.chars();
    let starts_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    // 含数字一律跳过：既排除格号（`A4`），也排除大概率不是列名的标识符。
    // 宁可漏报也不要误报——探针一旦误报，用户就会学会忽略它。
    let bare = starts_ok
        && argument
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !argument.chars().any(|c| c.is_ascii_digit());
    if !bare {
        return None;
    }
    Some(argument.to_string())
}

pub fn patch(j: &J, operations: &J) -> Result<J> {
    let patch: json_patch::Patch = serde_json::from_value(operations.clone())?;
    let mut changed = j.clone();
    json_patch::patch(&mut changed, &patch)
        .map_err(|e| Error::new(3, "PATCH_INVALID", "patch", e))?;
    validate(&changed, None)?;
    Ok(changed)
}
pub fn address(row: u64, col: u64) -> String {
    let mut n = col;
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    format!("{}{row}", letters.iter().rev().collect::<String>())
}
pub fn coord(s: &str) -> Result<(u64, u64)> {
    let p = s
        .find(|c: char| c.is_ascii_digit())
        .ok_or_else(|| Error::new(2, "USAGE", "arguments", "需要格号，如 A1"))?;
    let mut col = 0u64;
    for c in s[..p].bytes() {
        if !c.is_ascii_alphabetic() {
            return Err(Error::new(2, "USAGE", "arguments", "无效格号"));
        }
        col = col
            .checked_mul(26)
            .and_then(|n| n.checked_add((c.to_ascii_uppercase() - b'A' + 1) as u64))
            .ok_or_else(|| Error::new(2, "USAGE", "arguments", "格号过大"))?;
    }
    let row = s[p..]
        .parse::<u64>()
        .map_err(|_| Error::new(2, "USAGE", "arguments", "无效行号"))?;
    if row == 0 || col == 0 {
        return Err(Error::new(2, "USAGE", "arguments", "坐标从 1 开始"));
    }
    Ok((row, col))
}
