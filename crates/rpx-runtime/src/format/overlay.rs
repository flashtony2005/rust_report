//! 字段级 legacy overlay（Phase 2）。
//!
//! 迁移快照不只用于“原样回写”：解析 JSON 后，把 v1 未建模的字段从原始
//! `ReportDefine` 合并回来。合并策略是“以原模型为底，用 JSON 模型的**已建模
//! 字段**覆盖”，这样既能编辑 v1 字段，又不会丢掉 diagonal / ois_blob /
//! exp_map / merge_null / dataset 高级槽 / print 未开放槽等。
//!
//! ⚠ 这不是逐字段枚举；它通过整 struct clone + 显式覆盖已建模槽实现。新增
//! v1 字段时必须同步这里的 `merge_*`，否则会被原值覆盖。

use std::collections::HashMap;

use rpx_core::{
    cell::Cell, colcell::ColCell, dataset::DataSetEntry, props::bit, reader::ObjValue,
    report::ReportDefine, rowcell::RowCell, CellId,
};
use serde_json::Value as J;

use super::legacy;

pub(super) fn apply(doc: &J, rd: &mut ReportDefine) {
    let Some(snapshot) = legacy::read(doc) else {
        return;
    };
    let Ok(original) = rpx_core::open_bytes(&snapshot.bytes) else {
        return;
    };
    overlay_report(&original, rd);
    overlay_print(&original, rd);
    overlay_datasets(&original, rd);
    overlay_rows(&original, rd);
    overlay_cols(&original, rd);
    overlay_cells(&original, rd);
}

fn overlay_report(original: &ReportDefine, rd: &mut ReportDefine) {
    rd.exp_map = original.exp_map.clone();
    rd.scalars = original.scalars.clone();
    rd.macro_meta = original.macro_meta.clone();
    rd.sub_report_meta = original.sub_report_meta.clone();
    rd.back_graph_config = original.back_graph_config.clone();
    rd.export_config = original.export_config.clone();

    // JSON 可编辑的已知 custom-properties 优先；其余原始键补回。
    let known = |key: &str| {
        key.starts_with("rust_report.macro.")
            || key.starts_with(rpx_engine::pager::options::PREFIX)
            || key == "rust_report.print.rows"
            || key.starts_with("rust_report.print.group.")
            || key == rpx_engine::graphics::BACKGROUND
    };
    for (key, value) in &original.custom_properties {
        if !known(key) && !rd.custom_properties.iter().any(|(k, _)| k == key) {
            rd.custom_properties.push((key.clone(), value.clone()));
        }
    }
}

fn overlay_rows(original: &ReportDefine, rd: &mut ReportDefine) {
    for (i, parsed) in rd.row_cells.iter_mut().enumerate() {
        let Some(orig) = original.row_cells.get(i) else {
            continue;
        };
        *parsed = merge_row(orig, parsed);
    }
}

fn merge_row(orig: &RowCell, parsed: &RowCell) -> RowCell {
    let mut out = orig.clone();
    out.row_type = parsed.row_type;
    out.row_height = parsed.row_height;
    out.row_visible = parsed.row_visible;
    out.break_page = parsed.break_page;
    out.break_column = parsed.break_column;
    out.adjust_row_h = parsed.adjust_row_h;
    out.group_header_level = parsed.group_header_level;
    out.exp_map = merge_exp_map(
        orig.exp_map.as_ref(),
        parsed.exp_map.as_ref(),
        &[1, 2, 7, 13, 15],
    );
    out
}

fn overlay_cols(original: &ReportDefine, rd: &mut ReportDefine) {
    for (i, parsed) in rd.col_cells.iter_mut().enumerate() {
        let Some(orig) = original.col_cells.get(i) else {
            continue;
        };
        *parsed = merge_col(orig, parsed);
    }
}

fn merge_col(orig: &ColCell, parsed: &ColCell) -> ColCell {
    let mut out = orig.clone();
    out.col_type = parsed.col_type;
    out.col_width = parsed.col_width;
    out.col_visible = parsed.col_visible;
    out.break_page = parsed.break_page;
    out.break_column = parsed.break_column;
    out.exp_map = merge_exp_map(
        orig.exp_map.as_ref(),
        parsed.exp_map.as_ref(),
        &[11, 12, 13, 15],
    );
    out
}

fn overlay_print(original: &ReportDefine, rd: &mut ReportDefine) {
    let o = &original.print_setup;
    let p = &rd.print_setup;
    let mut out = o.clone();
    out.paper = p.paper;
    out.table_column_num = p.table_column_num;
    out.layout_rows = p.layout_rows;
    out.layout_cols = p.layout_cols;
    out.row_num_per_page = p.row_num_per_page;
    out.zoom_scale = p.zoom_scale;
    out.paper_width = p.paper_width;
    out.paper_height = p.paper_height;
    out.left_margin = p.left_margin;
    out.right_margin = p.right_margin;
    out.top_margin = p.top_margin;
    out.bottom_margin = p.bottom_margin;
    out.back_graph_printed = p.back_graph_printed;
    out.column_repeated_group_header = p.column_repeated_group_header;
    out.copy_count = p.copy_count;
    out.full_screen_preview = p.full_screen_preview;
    out.blank_line_filled = p.blank_line_filled;
    out.pager_style = p.pager_style;
    out.virtual_printer = p.virtual_printer.clone();
    rd.print_setup = out;
}

fn overlay_datasets(original: &ReportDefine, rd: &mut ReportDefine) {
    for parsed in &mut rd.datasets {
        let Some(orig) = find_dataset(original, dataset_name(parsed).as_deref()) else {
            continue;
        };
        if let (Some(o), Some(p)) = (&orig.file, &parsed.file) {
            let mut file = o.clone();
            file.name = p.name.clone();
            file.file_name = p.file_name.clone();
            file.sheet_name = p.sheet_name.clone();
            file.encoding = p.encoding.clone();
            file.seperator = p.seperator;
            parsed.file = Some(file);
        } else if let (Some(o), Some(p)) = (&orig.sql, &parsed.sql) {
            let mut sql = o.clone();
            sql.name = p.name.clone();
            sql.data_source_name = p.data_source_name.clone();
            sql.sql = p.sql.clone();
            parsed.sql = Some(sql);
        }
        parsed.raw = orig.raw.clone();
    }
}

fn find_dataset<'a>(rd: &'a ReportDefine, name: Option<&str>) -> Option<&'a DataSetEntry> {
    let name = name?;
    rd.datasets
        .iter()
        .find(|d| dataset_name(d).as_deref() == Some(name))
}

fn dataset_name(d: &DataSetEntry) -> Option<String> {
    d.file
        .as_ref()
        .and_then(|f| f.name.clone().or_else(|| f.file_name.clone()))
        .or_else(|| d.sql.as_ref().and_then(|s| s.name.clone()))
}

fn overlay_cells(original: &ReportDefine, rd: &mut ReportDefine) {
    // 每个 parsed CellId 选首个坐标作为代表，用原坐标的未建模字段合并。
    let mut coords: HashMap<CellId, Vec<(usize, usize)>> = HashMap::new();
    for (r, row) in rd.grid.iter().enumerate() {
        for (c, slot) in row.iter().enumerate() {
            if let Some(id) = slot {
                coords.entry(*id).or_default().push((r, c));
            }
        }
    }

    let mut new_ids = HashMap::new();
    for (parsed_id, places) in &coords {
        let Some(&(r, c)) = places.first() else {
            continue;
        };
        let (Some(parsed), Some(original_cell)) = (rd.cell_at(r, c), original.cell_at(r, c)) else {
            continue;
        };
        let merged = merge_cell(original_cell, parsed);
        let id = rd.store.len();
        rd.store.push(merged);
        new_ids.insert(*parsed_id, id);
    }
    for (parsed_id, places) in coords {
        let Some(&id) = new_ids.get(&parsed_id) else {
            continue;
        };
        for (r, c) in places {
            if let Some(slot) = rd.grid.get_mut(r).and_then(|row| row.get_mut(c)) {
                *slot = Some(id);
            }
        }
    }

    // 合并区延续槽：原文件可能有独立格（不是锚点别名），JSON parser 会建成别名。
    // 这类格要单独克隆原格，不能跟随锚点的 merged_area。
    for r in 1..rd.rows {
        for c in 1..rd.cols {
            if r >= original.rows || c >= original.cols {
                continue;
            }
            let Some(orig_id) = original.grid[r][c] else {
                continue;
            };
            let (ar, ac) = anchor(original, r, c);
            let Some(orig_anchor) = original.grid[ar][ac] else {
                continue;
            };
            if orig_id == orig_anchor {
                continue;
            }
            let Some(orig_cell) = original.cell_at(r, c) else {
                continue;
            };
            // JSON 的合并延续槽可能是 None（不是别名）；此时原格就是该槽的
            // 唯一来源，直接克隆，不能用锚点的 modeled 字段覆盖它。
            let mut merged = match rd.cell_at(r, c) {
                Some(parsed_cell) => merge_cell(orig_cell, parsed_cell),
                None => orig_cell.clone(),
            };
            merged.merged_area = orig_cell.merged_area;
            let id = rd.store.len();
            rd.store.push(merged);
            if let Some(slot) = rd.grid.get_mut(r).and_then(|row| row.get_mut(c)) {
                *slot = Some(id);
            }
        }
    }
}

fn anchor(rd: &ReportDefine, r: usize, c: usize) -> (usize, usize) {
    rd.merges
        .iter()
        .find(|m| m.begin_row <= r && r <= m.end_row && m.begin_col <= c && c <= m.end_col)
        .map(|m| (m.begin_row, m.begin_col))
        .unwrap_or((r, c))
}

fn merge_cell(orig: &Cell, parsed: &Cell) -> Cell {
    let mut out = orig.clone();
    out.merged_area = parsed.merged_area;
    out.h_align = parsed.h_align;
    out.v_align = parsed.v_align;
    out.extend_mode = parsed.extend_mode;
    out.left_head = parsed.left_head.clone();
    out.top_head = parsed.top_head.clone();
    out.hyperlink = parsed.hyperlink.clone();
    out.hyperlink_window = parsed.hyperlink_window.clone();
    out.notes = parsed.notes.clone();
    out.value = parsed.value.clone();
    out.disp_value = parsed.disp_value.clone();
    out.indent = parsed.indent;
    out.font_name = parsed.font_name.clone();
    out.font_size = parsed.font_size;
    out.format = parsed.format.clone();
    out.fore_color = parsed.fore_color;
    out.back_color = parsed.back_color;
    out.lb_style = parsed.lb_style;
    out.lb_width = parsed.lb_width;
    out.lb_color = parsed.lb_color;
    out.rb_style = parsed.rb_style;
    out.rb_width = parsed.rb_width;
    out.rb_color = parsed.rb_color;
    out.tb_style = parsed.tb_style;
    out.tb_width = parsed.tb_width;
    out.tb_color = parsed.tb_color;
    out.bb_style = parsed.bb_style;
    out.bb_width = parsed.bb_width;
    out.bb_color = parsed.bb_color;
    out.diagonal_style = parsed.diagonal_style;
    out.diagonal_width = parsed.diagonal_width;
    out.diagonal_color = parsed.diagonal_color;
    out.merge_same = parsed.merge_same;
    out.rotation_angle = parsed.rotation_angle;
    out.exp_map = merge_exp_map(orig.exp_map.as_ref(), parsed.exp_map.as_ref(), &[40]);
    let known = bit::BOLD | bit::ITALIC | bit::UNDERLINE | bit::SPLITTED | bit::STRETCH_WHEN_PAGED;
    out.bit_flags = (orig.bit_flags & !known) | (parsed.bit_flags & known);
    out
}

fn merge_exp_map(
    orig: Option<&Vec<(u8, ObjValue)>>,
    parsed: Option<&Vec<(u8, ObjValue)>>,
    editable: &[u8],
) -> Option<Vec<(u8, ObjValue)>> {
    let mut out: Vec<(u8, ObjValue)> = orig.cloned().unwrap_or_default();
    out.retain(|(key, _)| !editable.contains(key));
    if let Some(parsed) = parsed {
        for (key, value) in parsed {
            if editable.contains(key) {
                out.push((*key, value.clone()));
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
