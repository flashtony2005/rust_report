//! Cell 位标志访问器与默认值判定（从 cell.rs 拆出，保持单文件 ≤300 行门禁）。
//!
//! 位标志表依据契约附件 B.3.1（默认 0x0020 = 仅 visible 置位）；
//! is_default 依据附件 B.3 各表默认值列（RUNLOG §4.2 默认清单的超集）。

use super::Cell;
use crate::props::{bit, def};
use crate::reader::ObjValue;

impl Cell {
    // ---------- 位标志访问器（B.3.1） ----------
    pub fn has(&self, mask: u16) -> bool {
        self.bit_flags & mask != 0
    }
    pub fn text_wrap(&self) -> bool { self.has(bit::TEXT_WRAP) }
    pub fn visible(&self) -> bool { self.has(bit::VISIBLE) }
    pub fn bold(&self) -> bool { self.has(bit::BOLD) }
    pub fn italic(&self) -> bool { self.has(bit::ITALIC) }
    pub fn underline(&self) -> bool { self.has(bit::UNDERLINE) }
    pub fn row_hidden(&self) -> bool { self.has(bit::ROW_HIDDEN) }
    pub fn col_hidden(&self) -> bool { self.has(bit::COL_HIDDEN) }
    pub fn back_diagonal(&self) -> bool { self.has(bit::BACK_DIAGONAL) }

    /// 与 expected.json 收录规则对齐：全部字段等于契约默认值时为 true
    /// （RUNLOG §4.2 默认清单的超集，含附件 B 全部默认值）。
    pub fn is_default(&self) -> bool {
        self.merged_area.is_none()
            && self.h_align == def::HALIGN_LEFT
            && self.v_align == def::VALIGN_MIDDLE
            && self.diagonal_style == 0
            && self.diagonal_color == def::COLOR_BLACK
            && (self.diagonal_width - def::DIAGONAL_WIDTH).abs() < 1e-5
            && self.extend_mode == def::EXTEND_DEFAULT
            && self.extend_area.is_none()
            && self.left_head.is_none()
            && self.top_head.is_none()
            && self.to_excel_mode == def::TO_EXCEL_MODE
            && self.adjust_size_mode == def::ADJUST_SIZE_MODE
            && self.cell_obj.is_none()
            && self.value == ObjValue::Null
            && self.disp_value.is_none()
            && (self.indent - def::INDENT).abs() < 1e-5
            && font_is_default(&self.font_name)
            && self.font_size == def::FONT_SIZE
            && self.format.is_none()
            && self.fore_color == def::COLOR_BLACK
            && self.back_color == def::COLOR_WHITE
            && self.hyperlink.is_none()
            && self.hyperlink_window.is_none()
            && self.lb_style == 0
            && (self.lb_width - def::BORDER_WIDTH).abs() < 1e-5
            && self.lb_color == def::COLOR_BLACK
            && self.rb_style == 0
            && (self.rb_width - def::BORDER_WIDTH).abs() < 1e-5
            && self.rb_color == def::COLOR_BLACK
            && self.tb_style == 0
            && (self.tb_width - def::BORDER_WIDTH).abs() < 1e-5
            && self.tb_color == def::COLOR_BLACK
            && self.bb_style == 0
            && (self.bb_width - def::BORDER_WIDTH).abs() < 1e-5
            && self.bb_color == def::COLOR_BLACK
            && self.exp_map.is_none()
            && self.bit_flags == bit::DEFAULT_FLAGS
            && self.cell_type == def::CELL_TYPE
            && self.notes.is_none()
            && ois_is_default(&self.ois_blob)
            && self.cell_graph_config.is_none()
            && self.under_line_style == 0
            && self.cell_style_name.is_none()
            && self.merge_same == def::MERGE_SAME
            && self.merge_null == def::MERGE_NULL
            && self.merge_same_mode == def::MERGE_SAME_MODE
            && self.merge_null_mode == def::MERGE_NULL_MODE
            && (self.rotation_angle - def::ROTATION_ANGLE).abs() < 1e-5
    }
}

fn font_is_default(name: &Option<String>) -> bool {
    match name {
        None => true,
        Some(s) => s == def::DEFAULT_FONT_NAME,
    }
}

fn ois_is_default(blob: &Option<Vec<u8>>) -> bool {
    match blob {
        None => true,
        Some(b) => b.len() == def::OIS_DEFAULT_LEN && b[..] == def::OIS_DEFAULT_BYTES,
    }
}
