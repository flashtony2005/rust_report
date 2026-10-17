//! 数据集投影。

use rpx_core::{dataset::FileDataSetConfig, ReportDefine};
use serde_json::{json, Value as J};

pub(super) fn datasets(rd: &ReportDefine, warnings: &mut Vec<String>) -> Option<Vec<J>> {
    if rd.datasets.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for (i, d) in rd.datasets.iter().enumerate() {
        if let Some(file) = &d.file {
            if let Some(value) = file_json(file, warnings, i) {
                out.push(value);
            }
        } else if let Some(sql) = &d.sql {
            let (Some(name), Some(dsn), Some(text)) = (
                sql.name.as_ref(),
                sql.data_source_name.as_ref(),
                sql.sql.as_ref(),
            ) else {
                warnings.push(format!("datasets/{i}"));
                continue;
            };
            if sql.schema.is_some()
                || sql.params.is_some()
                || sql.begin_row.is_some()
                || sql.end_row.is_some()
                || sql.col_titles.is_some()
                || sql.is_guarded
                || sql.threshold != -1
            {
                warnings.push(format!("datasets/{i}/legacy"));
            }
            out.push(json!({"name": name, "sql": {"dsn": dsn, "sql": text}}));
        } else {
            warnings.push(format!("datasets/{i}/class_name"));
        }
    }
    Some(out)
}

fn file_json(f: &FileDataSetConfig, warnings: &mut Vec<String>, i: usize) -> Option<J> {
    let file_name = f.file_name.as_ref()?;
    let mut file = json!({"file_name": file_name});
    if let Some(v) = &f.encoding {
        file["encoding"] = json!(v);
    }
    if let Some(v) = &f.sheet_name {
        file["sheet_name"] = json!(v);
    }
    if let Some(ch) = char::from_u32(f.seperator as u32) {
        file["delimiter"] = json!(ch.to_string());
    } else {
        warnings.push(format!("datasets/{i}/delimiter"));
    }
    if f.data_source_name.is_some()
        || f.col_list.is_some()
        || !f.title
        || f.quote_removed
        || f.col_type_list.is_some()
        || f.col_format_list.is_some()
        || f.filter.is_some()
        || f.gather_col_list.is_some()
        || f.gather_fun_list.is_some()
        || f.group_col_list.is_some()
        || f.select_col_list.is_some()
        || f.change_file
        || f.begin_row.is_some()
        || f.end_row.is_some()
        || f.col_titles.is_some()
        || f.cache_data
        || f.release_mode != 0
        || f.no_data_prompt
    {
        warnings.push(format!("datasets/{i}/legacy"));
    }
    let mut value = json!({"file": file});
    if let Some(name) = &f.name {
        value["name"] = json!(name);
    }
    Some(value)
}
