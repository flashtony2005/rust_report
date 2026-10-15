//! 各记录的**写侧**：与 `cell.rs` / `rowcell.rs` / `colcell.rs` / `print_setup.rs` /
//! `param.rs` / `dataset.rs` 的 `read` 逐槽互逆。
//!
//! 槽序唯一事实源是 `docs/format-contract.md` 附件 B（单元格）/ D（PrintSetup）/
//! E（Param 与数据集）。本文件的每条 `write_*` 都对应读侧同名 `read`，
//! **改一处必须同时改另一处** —— 偏离会被 `tests/roundtrip.rs` 的逐夹具字节对拍抓住。

use crate::cell::{Area, Cell};
use crate::colcell::ColCell;
use crate::dataset::{DataSetEntry, FileDataSetConfig, SqlDataSetConfig};
use crate::error::Result;
use crate::param::Param;
use crate::print_setup::PrintSetup;
use crate::rowcell::RowCell;

use super::{record_body, RecordWriter};

/// `Area`：4×int32，顺序 **r1, r2, c1, c2**（附件 B.3.3 第 3 条 —— 不是 r1,c1,r2,c2）。
pub(super) fn write_area(w: &mut RecordWriter, a: &Area) {
    w.write_i32(a.begin_row);
    w.write_i32(a.end_row);
    w.write_i32(a.begin_col);
    w.write_i32(a.end_col);
}

/// RowCell：10 槽（附件 B.1）。
pub(super) fn write_row_cell(w: &mut RecordWriter, c: &RowCell) -> Result<()> {
    w.write_i8(c.row_type);
    w.write_f32(c.row_height);
    w.write_bool(c.row_visible);
    w.write_bool(c.break_page);
    match &c.exp_map {
        Some(e) => w.write_record(Some(&record_body(|s| s.write_byte_map(e))?)),
        None => w.write_record(None),
    }
    w.write_string(c.notes.as_deref());
    w.write_i8(c.group_header_level);
    w.write_blob(c.ois_blob.as_deref());
    w.write_bool(c.break_column);
    w.write_bool(c.adjust_row_h);
    Ok(())
}

/// ColCell：8 槽（附件 B.2）。
pub(super) fn write_col_cell(w: &mut RecordWriter, c: &ColCell) -> Result<()> {
    w.write_i8(c.col_type);
    w.write_f32(c.col_width);
    w.write_bool(c.col_visible);
    w.write_bool(c.break_page);
    match &c.exp_map {
        Some(e) => w.write_record(Some(&record_body(|s| s.write_byte_map(e))?)),
        None => w.write_record(None),
    }
    w.write_string(c.notes.as_deref());
    w.write_blob(c.ois_blob.as_deref());
    w.write_bool(c.break_column);
    Ok(())
}

/// NormalCell：49 槽（附件 B.3.2，与 `Cell::read` 逐槽互逆）。
pub(super) fn write_cell(w: &mut RecordWriter, c: &Cell) -> Result<()> {
    // 槽 1 mergedArea
    match &c.merged_area {
        Some(a) => w.write_record(Some(&record_body(|s| {
            write_area(s, a);
            Ok(())
        })?)),
        None => w.write_record(None),
    }
    w.write_i8(c.h_align);
    w.write_i8(c.v_align);
    w.write_u8(c.diagonal_style);
    w.write_i32(c.diagonal_color);
    w.write_f32(c.diagonal_width);
    w.write_i8(c.extend_mode);
    // 槽 8 extendArea
    match &c.extend_area {
        Some(a) => w.write_record(Some(&record_body(|s| {
            write_area(s, a);
            Ok(())
        })?)),
        None => w.write_record(None),
    }
    w.write_string(c.left_head.as_deref());
    w.write_string(c.top_head.as_deref());
    w.write_i8(c.to_excel_mode);
    w.write_i8(c.adjust_size_mode);
    // 槽 13 cellObj：false 时只 1 字节
    match &c.cell_obj {
        Some(o) => {
            w.write_bool(true);
            w.write_string(o.class_name.as_deref());
            w.write_record(Some(&o.raw));
        }
        None => w.write_bool(false),
    }
    // 槽 14 VALUE
    w.write_object(&c.value)?;
    w.write_string(c.disp_value.as_deref());
    w.write_f32(c.indent);
    w.write_string(c.font_name.as_deref());
    w.write_i16(c.font_size);
    w.write_string(c.format.as_deref());
    w.write_i32(c.fore_color);
    w.write_i32(c.back_color);
    w.write_string(c.hyperlink.as_deref());
    w.write_string(c.hyperlink_window.as_deref());
    w.write_u8(c.lb_style);
    w.write_f32(c.lb_width);
    w.write_i32(c.lb_color);
    w.write_u8(c.rb_style);
    w.write_f32(c.rb_width);
    w.write_i32(c.rb_color);
    w.write_u8(c.tb_style);
    w.write_f32(c.tb_width);
    w.write_i32(c.tb_color);
    w.write_u8(c.bb_style);
    w.write_f32(c.bb_width);
    w.write_i32(c.bb_color);
    // 槽 36 expMap
    match &c.exp_map {
        Some(e) => w.write_record(Some(&record_body(|s| s.write_byte_map(e))?)),
        None => w.write_record(None),
    }
    w.write_i16(c.bit_flags as i16);
    w.write_i8(c.cell_type);
    w.write_string(c.notes.as_deref());
    // 槽 40：writeStrings(null) 的 2 字节空洞（附件 A.3）
    w.write_strings(None);
    w.write_blob(c.ois_blob.as_deref());
    w.write_blob(c.cell_graph_config.as_deref());
    w.write_u8(c.under_line_style);
    w.write_string(c.cell_style_name.as_deref());
    w.write_i8(c.merge_same);
    w.write_i8(c.merge_null);
    w.write_i8(c.merge_same_mode);
    w.write_i8(c.merge_null_mode);
    // 槽 49：写侧**无条件**写出（附件 A.6）
    w.write_f32(c.rotation_angle);
    Ok(())
}

/// PrintSetup：33 槽（附件 D）。槽 31-33 读侧有 `available()` 守卫，写侧无条件写出。
pub(super) fn write_print_setup(w: &mut RecordWriter, p: &PrintSetup) {
    w.write_i16(p.paper);
    w.write_i16(p.table_column_num);
    w.write_i16(p.layout_rows);
    w.write_i16(p.layout_cols);
    w.write_i32(p.row_num_per_page);
    w.write_i16(p.zoom_page_width);
    w.write_i16(p.zoom_page_height);
    w.write_i16(p.zoom_scale);
    w.write_f32(p.paper_width);
    w.write_f32(p.paper_height);
    w.write_f32(p.left_margin);
    w.write_f32(p.right_margin);
    w.write_f32(p.top_margin);
    w.write_f32(p.bottom_margin);
    w.write_i8(p.orient);
    w.write_i8(p.layout);
    w.write_i8(p.page_header_and_footer_y_mode);
    w.write_i8(p.page_header_and_footer_x_mode);
    w.write_i8(p.row_table_header_and_footer_mode);
    w.write_i8(p.title_y_mode);
    w.write_i8(p.title_x_mode);
    w.write_i8(p.col_table_header_and_footer_mode);
    w.write_i8(p.pager_style);
    w.write_i8(p.zoom_mode);
    w.write_i8(p.h_align);
    w.write_i8(p.v_align);
    w.write_bool(p.back_graph_printed);
    w.write_string(p.virtual_printer.as_deref());
    w.write_bool(p.column_repeated_group_header);
    w.write_i32(p.copy_count);
    w.write_bool(p.full_screen_preview);
    w.write_bool(p.blank_line_filled);
    w.write_i8(p.column_mode);
}

/// Param：10 槽（附件 E.1）。读侧丢弃的 3 个槽按写侧原样补默认（pad=0、两个 short=0、
/// 两处 null 占位 = int32 -1）。
pub(super) fn write_param(w: &mut RecordWriter, p: &Param) {
    w.write_i8(p.param_type);
    w.write_i8(p.data_type);
    w.write_i8(0); // 槽 3 pad（读侧丢弃）
    w.write_string(p.name.as_deref());
    w.write_string(p.description.as_deref());
    w.write_string(None); // 槽 6 null 占位（读侧丢弃）
    w.write_string(p.value.as_deref());
    w.write_i16(0); // 槽 8（读侧丢弃）
    w.write_i16(0); // 槽 9（读侧丢弃）
    w.write_string(None); // 槽 10 null 占位（读侧丢弃）
}

/// ParamMetaData 记录体：int16 个数 + 逐条 `writeRecord(Param)`（附件 E.2）。
pub(super) fn write_param_meta(params: &[Param]) -> Result<Vec<u8>> {
    record_body(|w| {
        w.write_i16(params.len() as i16);
        for p in params {
            let body = record_body(|s| {
                write_param(s, p);
                Ok(())
            })?;
            w.write_record(Some(&body));
        }
        Ok(())
    })
}

/// DataSetConfig 基类 6 槽（附件 E.3）—— `FileDataSetConfig` 与 `SqlDataSetConfig` 共用。
#[allow(clippy::too_many_arguments)]
fn write_base(
    w: &mut RecordWriter,
    name: Option<&str>,
    data_source_name: Option<&str>,
    release_mode: i8,
    cache_data: bool,
    col_titles: Option<&[Option<String>]>,
    no_data_prompt: bool,
) {
    w.write_string(name);
    w.write_string(data_source_name);
    w.write_i8(release_mode);
    w.write_bool(cache_data);
    w.write_strings(col_titles);
    w.write_bool(no_data_prompt);
}

/// `FileDataSetConfig`（附件 E.4）：基类 6 槽包在 int32 长度 blob 里 + 文件槽。
///
/// 尾段（槽 14-23）读侧有 `available()` 守卫；写侧**全部无条件写出**
/// （新 writer 行为，与 `SqlDataSetConfig` 同口径）。
pub(super) fn write_file_config(w: &mut RecordWriter, cfg: &FileDataSetConfig) -> Result<()> {
    let base = record_body(|s| {
        write_base(
            s,
            cfg.name.as_deref(),
            cfg.data_source_name.as_deref(),
            cfg.release_mode,
            cfg.cache_data,
            cfg.col_titles.as_deref(),
            cfg.no_data_prompt,
        );
        Ok(())
    })?;
    w.write_blob(Some(&base));
    w.write_string(cfg.file_name.as_deref());
    w.write_string(cfg.sheet_name.as_deref());
    w.write_string(cfg.encoding.as_deref());
    w.write_strings(cfg.col_list.as_deref());
    w.write_bool(cfg.title);
    w.write_bool(cfg.quote_removed);
    w.write_i16(cfg.seperator);
    let col_types = cfg.col_type_list.as_deref().unwrap_or(&[]);
    w.write_i16(col_types.len() as i16);
    for v in col_types {
        w.write_i32(*v);
    }
    w.write_strings(cfg.col_format_list.as_deref());
    w.write_string(cfg.filter.as_deref());
    w.write_strings(cfg.gather_col_list.as_deref());
    w.write_strings(cfg.gather_fun_list.as_deref());
    w.write_strings(cfg.group_col_list.as_deref());
    w.write_strings(cfg.select_col_list.as_deref());
    w.write_bool(cfg.change_file);
    w.write_string(cfg.begin_row.as_deref());
    w.write_string(cfg.end_row.as_deref());
    Ok(())
}

/// `SQLDataSetConfig`（附件 E 的 SQL 版）：基类 blob + 平铺 8 槽。
pub(super) fn write_sql_config(w: &mut RecordWriter, cfg: &SqlDataSetConfig) -> Result<()> {
    let base = record_body(|s| {
        write_base(
            s,
            cfg.name.as_deref(),
            cfg.data_source_name.as_deref(),
            cfg.release_mode,
            cfg.cache_data,
            cfg.col_titles.as_deref(),
            cfg.no_data_prompt,
        );
        Ok(())
    })?;
    w.write_blob(Some(&base));
    w.write_string(cfg.sql.as_deref());
    w.write_string(cfg.begin_row.as_deref());
    w.write_string(cfg.end_row.as_deref());
    w.write_string(cfg.schema.as_deref());
    let params = cfg.params.as_deref().unwrap_or(&[]);
    w.write_i16(params.len() as i16);
    for (exp, ty) in params {
        w.write_string(exp.as_deref());
        w.write_i8(*ty);
    }
    w.write_bool(cfg.is_guarded);
    w.write_i32(cfg.threshold);
    Ok(())
}

/// 单条 DataSetEntry 的记录体。
///
/// **`raw` 优先**：从文件装载的条目保留了原始记录字节（含本结构未建模的槽与
/// available 守卫尾巴），原样回写才能字节复现；`raw` 为空（如 JSON 构造的定义）
/// 时回退到结构化序列化。
fn write_data_set_entry(w: &mut RecordWriter, e: &DataSetEntry) -> Result<()> {
    w.write_bool(true); // 条目存在标志（附件 E.5）
    w.write_string(Some(&e.class_name));
    let body = if !e.raw.is_empty() {
        e.raw.clone()
    } else {
        match (&e.file, &e.sql) {
            (Some(f), _) => record_body(|s| write_file_config(s, f))?,
            (_, Some(q)) => record_body(|s| write_sql_config(s, q))?,
            _ => Vec::new(),
        }
    };
    w.write_record(Some(&body));
    Ok(())
}

/// DataSetMetaData 记录体：int16 个数 + 逐条（附件 E.5）。
pub(super) fn write_data_set_meta(entries: &[DataSetEntry]) -> Result<Vec<u8>> {
    record_body(|w| {
        w.write_i16(entries.len() as i16);
        for e in entries {
            write_data_set_entry(w, e)?;
        }
        Ok(())
    })
}
