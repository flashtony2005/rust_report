use crate::{
    error::{Error, Result},
    parameters, reportdef,
};
use rpx_core::ReportDefine;
use rpx_engine::MacroTable;
use serde_json::{json, Value as J};
use std::collections::HashSet;

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
