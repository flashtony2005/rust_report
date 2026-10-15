//! `ReportDefine` 载荷装配：契约附件 C.2 的 **24 步字段序**（`report.rs::fill` 的逆）。
//!
//! # 唯一的非机械步骤：合并区「铺满」的逆运算
//!
//! 读侧（`report.rs` 步骤 5）在读到**锚点**时用 `fill_merge` 把整个矩形槽位都指向
//! 锚点的 `CellId`，之后行优先循环里**真实的非锚点记录会覆盖**它（Java `setCell` 语义，
//! 附件 E.8）。因此写侧必须反过来判断：
//!
//! * 槽里是 `None` → 写 `int32 -1`；
//! * 槽里是 `Some(id)` 且 `id` 恰是**覆盖本槽的某合并区锚点**，而本槽**不是锚点**
//!   → 这是 `fill_merge` 铺出来的，写 `int32 -1`（写锚点记录会得到不同的网格）；
//! * 其余 `Some(id)` → 写 `store[id]` 的完整记录（含 E.8 的「浅合并」形态：
//!   非锚点槽持有自己的默认格）。
//!
//! `rd.merges` **不直接落盘** —— 它由锚点格的 `merged_area` 派生（读侧同样重建）。

use crate::error::Result;
use crate::report::ReportDefine;

use super::records::{
    write_cell, write_col_cell, write_data_set_meta, write_param_meta, write_print_setup,
    write_row_cell,
};
use super::{record_body, RecordWriter};

/// 本槽是否由 `fill_merge` 从锚点铺出（而非流里真实存在的记录）。
fn is_fill_propagated(rd: &ReportDefine, id: usize, r: usize, c: usize) -> bool {
    rd.merges.iter().any(|m| {
        m.cell == id
            && (r, c) != (m.begin_row, m.begin_col)
            && r >= m.begin_row
            && r <= m.end_row
            && c >= m.begin_col
            && c <= m.end_col
    })
}

/// 七个 MetaData 之一：`Option` blob 记录。
///
/// ⚠ **空表写 `None`**：读侧 [`crate::reader::RecordReader::read_blob`] 把
/// `size <= 0` 与「空记录」都并成 `None`，模型无法区分二者；「没有参数 / 没有数据集」
/// 时写 int32 -1（= Java `writeRecord(null)`）是唯一能被读侧还原的形态。
fn write_meta_blob(w: &mut RecordWriter, body: Option<&[u8]>) {
    w.write_blob(body);
}

/// 24 步载荷装配。
pub fn write_payload(rd: &ReportDefine) -> Result<Vec<u8>> {
    let mut w = RecordWriter::new();

    // 步骤 1-2：矩阵维度
    w.write_i32(rd.rows as i32);
    w.write_i32(rd.cols as i32);

    // 步骤 3：RowCell（1..r-1）—— 写侧无条件写出（读侧不容忍空槽）
    for rc in &rd.row_cells {
        w.write_record(Some(&record_body(|s| write_row_cell(s, rc))?));
    }

    // 步骤 4：ColCell（1..c-1）
    for cc in &rd.col_cells {
        w.write_record(Some(&record_body(|s| write_col_cell(s, cc))?));
    }

    // 步骤 5：NormalCell（行优先）；空槽 = int32 -1
    for r in 1..rd.rows {
        for c in 1..rd.cols {
            match rd.grid[r][c] {
                None => w.write_record(None),
                Some(id) if is_fill_propagated(rd, id, r, c) => w.write_record(None),
                Some(id) => {
                    let body = record_body(|s| write_cell(s, &rd.store[id]))?;
                    w.write_record(Some(&body));
                }
            }
        }
    }

    // 步骤 6：七个 MetaData（PrintSetup 永远非空）
    let ps = record_body(|s| {
        write_print_setup(s, &rd.print_setup);
        Ok(())
    })?;
    w.write_record(Some(&ps));
    write_meta_blob(&mut w, rd.macro_meta.as_deref());
    let param_meta = if rd.params.is_empty() {
        None
    } else {
        Some(write_param_meta(&rd.params)?)
    };
    write_meta_blob(&mut w, param_meta.as_deref());
    let ds_meta = if rd.datasets.is_empty() {
        None
    } else {
        Some(write_data_set_meta(&rd.datasets)?)
    };
    write_meta_blob(&mut w, ds_meta.as_deref());
    write_meta_blob(&mut w, rd.sub_report_meta.as_deref());
    write_meta_blob(&mut w, rd.back_graph_config.as_deref());
    write_meta_blob(&mut w, rd.export_config.as_deref());

    // 步骤 7-16：10 个标量
    let s = &rd.scalars;
    w.write_i32(s.disp_ratio);
    w.write_i8(s.unit);
    w.write_string(s.tip.as_deref());
    w.write_i8(s.submit);
    w.write_string(s.notes.as_deref());
    w.write_string(s.future_cell_num_exp.as_deref());
    w.write_i32(s.image_max_width);
    w.write_i32(s.image_max_height);
    w.write_i8(s.page_border_mode);
    w.write_string(s.report_style_name.as_deref());

    // 步骤 17：cpSize 属性表
    w.write_i32(rd.custom_properties.len() as i32);
    for (k, v) in &rd.custom_properties {
        w.write_string(Some(k));
        w.write_string(v.as_deref());
    }

    // 步骤 18：_$5 expMap（ByteMap 记录）
    match &rd.exp_map {
        Some(e) => w.write_record(Some(&record_body(|s| s.write_byte_map(e))?)),
        None => w.write_record(None),
    }

    // 步骤 19-22
    w.write_string(s.chart_style_name.as_deref());
    w.write_string(s.validity_script.as_deref());
    w.write_string(s.big_data_set_name.as_deref());
    w.write_string(s.password.as_deref());

    // 步骤 23-24：流到此结束
    w.write_bool(s.high_precision);
    w.write_i32(s.threshold);

    Ok(w.into_bytes())
}
