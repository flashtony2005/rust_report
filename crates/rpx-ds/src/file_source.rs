//! 文件源物化：CSV/TSV 与 Excel 均输出 RowSet，计算引擎无需依赖文件解析器。
use std::{collections::HashSet, io::Cursor};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use rpx_core::{rowset::RowSet, value::Value};

pub fn is_excel(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "xlsx" | "xls" | "xlsm" | "xlsb" | "ods")
}

fn headers(cols: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    if cols.is_empty() {
        return Err("缺少列标题".into());
    }
    for name in &cols {
        if name.trim().is_empty() || !seen.insert(name.clone()) {
            return Err(format!("列标题为空或重复：{name:?}"));
        }
    }
    Ok(cols)
}

/// 首行列名；显式分隔符；引号、转义与引号内换行交给 csv crate。
pub fn delimited(text: &str, delimiter: u8) -> Result<RowSet, String> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .from_reader(text.trim_start_matches('\u{feff}').as_bytes());
    let cols = headers(
        reader
            .headers()
            .map_err(|e| e.to_string())?
            .iter()
            .map(str::to_string)
            .collect(),
    )?;
    let rows = reader
        .records()
        .map(|row| {
            row.map(|r| r.iter().map(scalar).collect())
                .map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    Ok(RowSet {
        name: String::new(),
        cols,
        rows,
    })
}

fn scalar(text: &str) -> Value {
    if text.is_empty() {
        return Value::Null;
    }
    if let Ok(n) = text.parse::<i32>() {
        return Value::Int(n);
    }
    if let Ok(n) = text.parse::<i64>() {
        return Value::Long(n);
    }
    if let Ok(n) = text.parse::<f64>() {
        if n.is_finite() {
            return Value::Double(n);
        }
    }
    match text {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::Text(text.into()),
    }
}

/// 选定工作表，未指定时使用第一张；读取缓存公式值，不执行 Excel 公式。
pub fn excel(bytes: &[u8], sheet: Option<&str>) -> Result<RowSet, String> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let sheet = sheet
        .map(str::to_string)
        .or_else(|| workbook.sheet_names().first().cloned())
        .ok_or("工作簿没有工作表")?;
    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|e| format!("工作表 {sheet}：{e}"))?;
    let formulas = workbook
        .worksheet_formula(&sheet)
        .map_err(|e| e.to_string())?;
    for (r, c, formula) in formulas.cells() {
        if !formula.is_empty() {
            let start = formulas.start().unwrap_or_default();
            let pos = (start.0 + r as u32, start.1 + c as u32);
            if matches!(range.get_value(pos), None | Some(Data::Empty)) {
                return Err(format!(
                    "工作表 {sheet} 第{}行第{}列公式没有缓存值，请在 Excel 中计算并保存",
                    pos.0 + 1,
                    pos.1 + 1
                ));
            }
        }
    }
    let mut input = range.rows();
    let cols = headers(
        input
            .next()
            .ok_or("工作表为空")?
            .iter()
            .map(|v| v.to_string())
            .collect(),
    )?;
    let mut rows = Vec::new();
    for (r, values) in input.enumerate() {
        if values.iter().all(|v| matches!(v, Data::Empty)) {
            continue;
        }
        let mut row = Vec::new();
        for (c, v) in values.iter().enumerate() {
            row.push(match v {
                Data::Empty => Value::Null,
                Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => {
                    Value::Text(s.clone())
                }
                Data::Int(n) => i32::try_from(*n).map(Value::Int).unwrap_or(Value::Long(*n)),
                Data::Float(n) if n.is_finite() => Value::Double(*n),
                Data::Bool(b) => Value::Bool(*b),
                Data::DateTime(dt) if dt.is_datetime() => {
                    let (y, m, d, h, min, s, ms) = dt.to_ymd_hms_milli();
                    Value::Text(format!(
                        "{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}.{ms:03}"
                    ))
                }
                other => {
                    return Err(format!(
                        "工作表 {sheet} 第{}行第{}列包含不支持的值 {other}",
                        r + 2,
                        c + 1
                    ))
                }
            });
        }
        row.resize(cols.len(), Value::Null);
        rows.push(row);
    }
    Ok(RowSet {
        name: String::new(),
        cols,
        rows,
    })
}
