//! 行/列属性投影。

use rpx_core::{props::def, rowcell::RowCell, ReportDefine};
use serde_json::{json, Value as J};

use super::naming::{col_type, row_type};

pub(super) fn row_props(rd: &ReportDefine, warnings: &mut Vec<String>) -> Option<Vec<J>> {
    if rd.row_cells.is_empty() {
        return None;
    }
    let mut rows = Vec::new();
    for (i, r) in rd.row_cells.iter().enumerate() {
        let row = i + 1;
        if row >= rd.rows {
            warnings.push(format!("row_props/{row}/index"));
            continue;
        }
        let Some(kind) = row_type(r.row_type) else {
            warnings.push(format!("row_props/{row}/type"));
            continue;
        };
        let mut p = json!({
            "row": row,
            "type": kind,
            "height": r.row_height,
            "visible": r.row_visible,
            "break_page": r.break_page,
            "break_column": r.break_column,
            "adjust_height": r.adjust_row_h,
        });
        if r.row_type == -90 && r.group_header_level > 0 {
            p["group_level"] = json!(r.group_header_level);
        }
        map_row_expressions(r, &mut p, row, warnings);
        if r.notes.is_some() || non_default_ois(&r.ois_blob) {
            warnings.push(format!("row_props/{row}/legacy"));
        }
        rows.push(p);
    }
    Some(rows)
}

fn map_row_expressions(r: &RowCell, p: &mut J, row: usize, warnings: &mut Vec<String>) {
    let Some(exp) = &r.exp_map else {
        return;
    };
    for (key, value) in exp {
        let name = match key {
            1 => "height",
            2 => "visible",
            7 => "adjust_height",
            13 => "break_page",
            15 => "break_column",
            _ => {
                warnings.push(format!("row_props/{row}/expressions/{key}"));
                continue;
            }
        };
        let rpx_core::reader::ObjValue::Str(value) = value else {
            warnings.push(format!("row_props/{row}/{name}"));
            continue;
        };
        p[name] = json!(value);
    }
}

pub(super) fn col_props(rd: &ReportDefine, warnings: &mut Vec<String>) -> Option<Vec<J>> {
    if rd.col_cells.is_empty() {
        return None;
    }
    let mut cols = Vec::new();
    for (i, c) in rd.col_cells.iter().enumerate() {
        let col = i + 1;
        if col >= rd.cols {
            warnings.push(format!("col_props/{col}/index"));
            continue;
        }
        let Some(kind) = col_type(c.col_type) else {
            warnings.push(format!("col_props/{col}/type"));
            continue;
        };
        let mut p = json!({
            "col": col,
            "type": kind,
            "width": c.col_width,
            "visible": c.col_visible,
            "break_page": c.break_page,
            "break_column": c.break_column,
        });
        if let Some(exp) = &c.exp_map {
            for (key, value) in exp {
                let name = match key {
                    11 => "width",
                    12 => "visible",
                    13 => "break_page",
                    15 => "break_column",
                    _ => {
                        warnings.push(format!("col_props/{col}/expressions/{key}"));
                        continue;
                    }
                };
                let rpx_core::reader::ObjValue::Str(value) = value else {
                    warnings.push(format!("col_props/{col}/{name}"));
                    continue;
                };
                p[name] = json!(value);
            }
        }
        if c.notes.is_some() || non_default_ois(&c.ois_blob) {
            warnings.push(format!("col_props/{col}/legacy"));
        }
        cols.push(p);
    }
    Some(cols)
}

pub(super) fn non_default_ois(blob: &Option<Vec<u8>>) -> bool {
    blob.as_deref() != Some(def::OIS_DEFAULT_BYTES.as_slice())
}
