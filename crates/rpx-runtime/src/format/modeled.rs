//! `.rpx` `ReportDefine` → v1 JSON 的可建模投影。
//!
//! 这里只输出 JSON v1 能表达的字段；v1 不表达的字段由 [`super::legacy`]
//! 以原始字节快照保存（Phase 1 迁移模式）。本模块的目标是：
//!
//! * 40 个产品 `.rpx` 都能产出一份**可被 `reportdef` 重新解析**的 JSON；
//! * 遇到未知码位时降级为“省略该字段 + 记录 warning”，而不是失败；
//! * `PreserveLegacy` 的严格无损由原始字节快照兜底，`Strict` 用 warning 拒绝。

use rpx_core::{
    cell::Cell,
    props::{bit, def},
    ReportDefine,
};
use serde_json::{json, Map, Value as J};

use crate::{definition::address, reportdef};

mod bands;
mod datasets;
mod naming;
mod params;
mod print;

/// 生成 v1 JSON 的可建模部分；`warnings` 里是“v1 表达不了、需要 legacy”的路径。
pub(super) fn document(rd: &ReportDefine) -> (J, Vec<String>) {
    let mut warnings = Vec::new();
    let rows = rd.rows.saturating_sub(1);
    let cols = rd.cols.saturating_sub(1);
    let mut cells = Vec::new();
    for row in 1..rd.rows {
        for col in 1..rd.cols {
            let Some(cell) = rd.cell_at(row, col) else {
                continue;
            };
            let is_merge_continuation = cell
                .merged_area
                .is_some_and(|a| a.begin_row as usize != row || a.begin_col as usize != col)
                || rd.merges.iter().any(|m| {
                    m.begin_row <= row
                        && row <= m.end_row
                        && m.begin_col <= col
                        && col <= m.end_col
                        && (m.begin_row != row || m.begin_col != col)
                });
            if is_merge_continuation {
                continue;
            }
            cells.push(cell_json(row, col, cell, &mut warnings));
        }
    }

    let mut doc = json!({
        "version": 1,
        "grid": {"rows": rows, "cols": cols},
        "cells": cells,
    });
    let obj = doc.as_object_mut().expect("document is object");

    if let Some(row_props) = bands::row_props(rd, &mut warnings) {
        obj.insert("row_props".into(), J::Array(row_props));
    }
    if let Some(col_props) = bands::col_props(rd, &mut warnings) {
        obj.insert("col_props".into(), J::Array(col_props));
    }
    if let Some(datasets) = datasets::datasets(rd, &mut warnings) {
        obj.insert("datasets".into(), J::Array(datasets));
    }
    if let Some(params) = params::params(rd, &mut warnings) {
        obj.insert("params".into(), J::Array(params));
    }
    if let Some(print_setup) = print::print_setup(rd, &mut warnings) {
        obj.insert("print_setup".into(), print_setup);
    }

    // custom properties → macros / printing / group_breaks / background。
    let mut macros = Map::new();
    let mut printing = Map::new();
    let mut groups = Vec::new();
    for (key, value) in &rd.custom_properties {
        let Some(value) = value.as_deref() else {
            warnings.push(format!("custom_properties.{key}"));
            continue;
        };
        if let Some(name) = key.strip_prefix("rust_report.macro.") {
            macros.insert(name.into(), json!(value));
        } else if let Some(name) = key.strip_prefix(rpx_engine::pager::options::PREFIX) {
            printing.insert(
                name.into(),
                serde_json::from_str(value).unwrap_or_else(|_| json!(value)),
            );
        } else if key == "rust_report.print.rows" {
            warnings.push("custom_properties.print.rows".into());
        } else if let Some(name) = key.strip_prefix("rust_report.print.group.") {
            groups.push(json!({
                "cell": name,
                "when": serde_json::from_str::<J>(value).unwrap_or_else(|_| json!(value)),
            }));
        } else if key == rpx_engine::graphics::BACKGROUND {
            obj.insert("background_image".into(), json!(value));
        } else {
            warnings.push(format!("custom_properties.{key}"));
        }
    }
    if !macros.is_empty() {
        obj.insert("macros".into(), J::Object(macros));
    }
    if !printing.is_empty() {
        obj.insert("printing".into(), J::Object(printing));
    }
    if !groups.is_empty() {
        obj.insert("group_breaks".into(), J::Array(groups));
    }

    // report 级 v1 不表达的字段：只做 warning（legacy 原始快照兜底）。
    if rd.exp_map.is_some() {
        warnings.push("exp_map".into());
    }
    if !scalars_are_default(&rd.scalars) {
        warnings.push("scalars".into());
    }
    for (name, value) in [
        ("macro_meta", &rd.macro_meta),
        ("sub_report_meta", &rd.sub_report_meta),
        ("back_graph_config", &rd.back_graph_config),
        ("export_config", &rd.export_config),
    ] {
        if value.is_some() {
            warnings.push(name.into());
        }
    }
    (doc, warnings)
}

fn cell_json(row: usize, col: usize, c: &Cell, warnings: &mut Vec<String>) -> J {
    let addr = address(row as u64, col as u64);
    let mut j = json!({"row": row, "col": col});

    if let Some(expr) = rpx_engine::cell_expression(c) {
        j["expr"] = json!(expr);
    } else if let Some(literal) = reportdef::literal::encode(&c.value) {
        j["literal"] = literal;
    } else {
        warnings.push(format!("cells/{addr}/value"));
    }
    if let Some(d) = &c.disp_value {
        j["disp_value"] = json!(d);
    }
    if let Some(v) = &c.font_name {
        j["font_name"] = json!(v);
    }
    if let Some(v) = &c.format {
        j["format"] = json!(v);
    }
    if let Some(v) = &c.left_head {
        j["left_head"] = json!(v);
    }
    if let Some(v) = &c.top_head {
        j["top_head"] = json!(v);
    }
    if let Some(v) = &c.hyperlink {
        j["hyperlink"] = json!(v);
    }
    if let Some(v) = &c.hyperlink_window {
        j["hyperlink_window"] = json!(v);
    }
    if let Some(v) = &c.notes {
        j["notes"] = json!(v);
    }
    j["font_size"] = json!(c.font_size);
    j["fore_color"] = json!(format!("#{:06X}", c.fore_color as u32 & 0xffffff));
    if c.back_color != -1 {
        j["back_color"] = json!(format!("#{:06X}", c.back_color as u32 & 0xffffff));
    }
    for (key, flag) in [
        ("bold", bit::BOLD),
        ("italic", bit::ITALIC),
        ("underline", bit::UNDERLINE),
        ("split", bit::SPLITTED),
        ("stretch_when_paged", bit::STRETCH_WHEN_PAGED),
    ] {
        j[key] = json!(c.bit_flags & flag != 0);
    }
    j["indent"] = json!(c.indent);
    j["rotation"] = json!(c.rotation_angle);

    if let Some(mode) = naming::extend_name(c.extend_mode) {
        j["extend_mode"] = json!(mode);
    } else {
        warnings.push(format!("cells/{addr}/extend_mode"));
    }
    if let Some(v) = naming::h_align(c.h_align) {
        j["h_align"] = json!(v);
    } else {
        warnings.push(format!("cells/{addr}/h_align"));
    }
    if let Some(v) = naming::v_align(c.v_align) {
        j["v_align"] = json!(v);
    } else {
        warnings.push(format!("cells/{addr}/v_align"));
    }
    if let Some(a) = c.merged_area {
        j["merge"] = json!({
            "rows": a.end_row - a.begin_row + 1,
            "cols": a.end_col - a.begin_col + 1,
        });
    }
    if let Some(mode) = naming::merge_same(c.merge_same) {
        j["merge_same"] = json!(mode);
    }
    if c.diagonal_style != 0 {
        j["diagonal"] = json!({
            "style": naming::border_style(c.diagonal_style),
            "width": c.diagonal_width,
            "color": format!("#{:06X}", c.diagonal_color as u32 & 0xffffff),
        });
    }
    let mut border = Map::new();
    for (key, style, width, color) in [
        ("left", c.lb_style, c.lb_width, c.lb_color),
        ("right", c.rb_style, c.rb_width, c.rb_color),
        ("top", c.tb_style, c.tb_width, c.tb_color),
        ("bottom", c.bb_style, c.bb_width, c.bb_color),
    ] {
        if let Some(style) = naming::border_style(style) {
            border.insert(
                key.into(),
                json!({
                    "style": style,
                    "width": width,
                    "color": format!("#{:06X}", color as u32 & 0xffffff),
                }),
            );
        } else {
            warnings.push(format!("cells/{addr}/border/{key}"));
        }
    }
    if !border.is_empty() {
        j["border"] = J::Object(border);
    }

    // 未建模字段：只在非默认时告警，避免默认 OIS blob 让 Strict 全红。
    if cell_has_legacy(c) {
        warnings.push(format!("cells/{addr}/legacy"));
    }
    j
}

fn cell_has_legacy(c: &Cell) -> bool {
    c.extend_area.is_some()
        || c.to_excel_mode != def::TO_EXCEL_MODE
        || c.adjust_size_mode != def::ADJUST_SIZE_MODE
        || c.cell_obj.is_some()
        || c.exp_map.is_some()
        || c.cell_type != def::CELL_TYPE
        || c.cell_graph_config.is_some()
        || c.under_line_style != 0
        || c.cell_style_name.is_some()
        || c.merge_null != def::MERGE_NULL
        || c.merge_same_mode != def::MERGE_SAME_MODE
        || c.merge_null_mode != def::MERGE_NULL_MODE
}

fn scalars_are_default(s: &rpx_core::report::ScalarTail) -> bool {
    s.disp_ratio == 133
        && s.unit == 1
        && s.tip.is_none()
        && s.submit == 0
        && s.notes.is_none()
        && s.future_cell_num_exp.is_none()
        && s.image_max_width == 0
        && s.image_max_height == 0
        && s.page_border_mode == 1
        && s.report_style_name.is_none()
        && s.chart_style_name.is_none()
        && s.validity_script.is_none()
        && s.big_data_set_name.is_none()
        && s.password.is_none()
        && s.high_precision
        && s.threshold == -1
}
