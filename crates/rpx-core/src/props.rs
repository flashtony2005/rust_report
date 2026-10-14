//! 紧凑流槽位常量与位标志。
//!
//! 契约依据 docs/format-contract.md 附件 B：
//! 属性码（0-7/10-15/20-94）**不在二进制流里**；本表按流序槽位编号（1 起），
//! 码位列仅作语义索引注释。B.3.1：位打包 short 共 14 个有效位，默认 0x0020（仅 visible）。

// ---------- RowCell：10 槽（B.1） ----------
pub mod row {
    pub const ROW_TYPE: usize = 1;
    pub const ROW_HEIGHT: usize = 2;
    pub const ROW_VISIBLE: usize = 3;
    pub const BREAK_PAGE: usize = 4;
    pub const EXP_MAP: usize = 5;
    pub const NOTES: usize = 6;
    pub const OIS_BLOB: usize = 7;
    pub const BREAK_COLUMN: usize = 8;
    pub const ADJUST_ROW_H: usize = 9;
    pub const SLOTS: usize = 10;
}

// ---------- ColCell：8 槽（B.2） ----------
pub mod col {
    pub const COL_TYPE: usize = 1;
    pub const COL_WIDTH: usize = 2;
    pub const COL_VISIBLE: usize = 3;
    pub const BREAK_PAGE: usize = 4;
    pub const EXP_MAP: usize = 5;
    pub const NOTES: usize = 6;
    pub const OIS_BLOB: usize = 7;
    pub const BREAK_COLUMN: usize = 8;
    pub const SLOTS: usize = 8;
}

// ---------- NormalCell：49 槽（B.3.2） ----------
pub mod cell {
    pub const MERGED_AREA: usize = 1;
    pub const HALIGN: usize = 2;
    pub const VALIGN: usize = 3;
    pub const DIAGONAL_STYLE: usize = 4;
    pub const DIAGONAL_COLOR: usize = 5;
    pub const DIAGONAL_WIDTH: usize = 6;
    pub const EXTEND_MODE: usize = 7;
    pub const EXTEND_AREA: usize = 8;
    pub const LEFT_HEAD: usize = 9;
    pub const TOP_HEAD: usize = 10;
    pub const TO_EXCEL_MODE: usize = 11;
    pub const ADJUST_SIZE_MODE: usize = 12;
    pub const CELL_OBJ: usize = 13;
    pub const VALUE: usize = 14;
    pub const DISP_VALUE: usize = 15;
    pub const INDENT: usize = 16;
    pub const FONT_NAME: usize = 17;
    pub const FONT_SIZE: usize = 18;
    pub const FORMAT: usize = 19;
    pub const FORE_COLOR: usize = 20;
    pub const BACK_COLOR: usize = 21;
    pub const HYPERLINK: usize = 22;
    pub const HYPERLINK_WINDOW: usize = 23;
    pub const LB_STYLE: usize = 24;
    pub const LB_WIDTH: usize = 25;
    pub const LB_COLOR: usize = 26;
    pub const RB_STYLE: usize = 27;
    pub const RB_WIDTH: usize = 28;
    pub const RB_COLOR: usize = 29;
    pub const TB_STYLE: usize = 30;
    pub const TB_WIDTH: usize = 31;
    pub const TB_COLOR: usize = 32;
    pub const BB_STYLE: usize = 33;
    pub const BB_WIDTH: usize = 34;
    pub const BB_COLOR: usize = 35;
    pub const EXP_MAP: usize = 36;
    pub const BIT_FLAGS: usize = 37;
    pub const CELL_TYPE: usize = 38;
    pub const NOTES: usize = 39;
    pub const STRINGS_HOLE: usize = 40; // writeStrings(null) 的 2 字节空洞
    pub const OIS_BLOB: usize = 41;
    pub const CELL_GRAPH_CONFIG: usize = 42;
    pub const UNDER_LINE_STYLE: usize = 43;
    pub const CELL_STYLE_NAME: usize = 44;
    pub const MERGE_SAME: usize = 45;
    pub const MERGE_NULL: usize = 46;
    pub const MERGE_SAME_MODE: usize = 47;
    pub const MERGE_NULL_MODE: usize = 48;
    pub const ROTATION_ANGLE: usize = 49; // 唯一 available() 守卫槽
    pub const SLOTS: usize = 49;
}

// ---------- 位打包 short（B.3.1，默认 0x0020） ----------
pub mod bit {
    pub const TEXT_WRAP: u16 = 0x0001; // bit0
    pub const SPLITTED: u16 = 0x0002; // bit1
    pub const BREAK_PAGE_AFTER_ROW: u16 = 0x0004; // bit2
    pub const BREAK_PAGE_AFTER_COL: u16 = 0x0008; // bit3
    pub const STRETCH_WHEN_PAGED: u16 = 0x0010; // bit4
    pub const VISIBLE: u16 = 0x0020; // bit5，默认置位
    pub const ROW_HIDDEN: u16 = 0x0040; // bit6
    pub const COL_HIDDEN: u16 = 0x0080; // bit7
    pub const BOLD: u16 = 0x0100; // bit8
    pub const ITALIC: u16 = 0x0200; // bit9
    pub const UNDERLINE: u16 = 0x0400; // bit10
    pub const BREAK_COLUMN_AFTER_ROW: u16 = 0x1000; // bit12
    pub const BREAK_COLUMN_AFTER_COL: u16 = 0x2000; // bit13
    pub const BACK_DIAGONAL: u16 = 0x4000; // bit14
    pub const DEFAULT_FLAGS: u16 = 0x0020;
}

// ---------- 默认值（B.1/B.2/B.3 各表"默认值"列） ----------
pub mod def {
    // RowCell
    pub const ROW_TYPE: i8 = -91;
    pub const ROW_HEIGHT: f32 = 8.0;
    // ColCell
    pub const COL_TYPE: i8 = -79;
    pub const COL_WIDTH: f32 = 25.0;
    // NormalCell
    pub const CELL_TYPE: i8 = -64;
    pub const HALIGN_LEFT: i8 = -48;
    pub const VALIGN_MIDDLE: i8 = -31;
    pub const EXTEND_DEFAULT: i8 = 16;
    pub const EXTEND_NONE: i8 = 17;
    pub const EXTEND_VERTICAL: i8 = 18;
    pub const EXTEND_HORIZONTAL: i8 = 19;
    pub const TO_EXCEL_MODE: i8 = 34;
    pub const ADJUST_SIZE_MODE: i8 = 49;
    pub const FONT_SIZE: i16 = 12;
    pub const DIAGONAL_WIDTH: f32 = 1.0;
    pub const INDENT: f32 = 0.0;
    pub const COLOR_BLACK: i32 = 0xFF00_0000u32 as i32; // -16777216
    pub const COLOR_WHITE: i32 = -1; // 0xFFFFFFFF
    pub const DEFAULT_FONT_NAME: &str = "宋体";
    pub const BORDER_WIDTH: f32 = 1.0;
    pub const MERGE_SAME: i8 = 97;
    pub const MERGE_NULL: i8 = 113;
    pub const MERGE_SAME_MODE: i8 = 118;
    pub const MERGE_NULL_MODE: i8 = 121;
    pub const ROTATION_ANGLE: f32 = 0.0;
    /// OIS blob 默认形态：两个 null IByteMap 经 ObjectOutputStream = 6 字节
    pub const OIS_DEFAULT_LEN: usize = 6;
    pub const OIS_DEFAULT_BYTES: [u8; 6] = [0xAC, 0xED, 0x00, 0x05, 0x70, 0x70];
}

/// 边框/斜线 style 字节读侧归一化：80（'P'）→ 0（附件 B.3.3 第 1 条）。
pub fn normalize_style(b: i8) -> u8 {
    if b == 80 {
        0
    } else {
        b as u8
    }
}

#[cfg(test)]
mod tests {
    use super::{bit, cell, col, def, normalize_style, row};

    /// 独立验证发现：把本函数改成恒等（去掉 80→0）后 7 个 golden 测试仍全绿 ——
    /// 夹具的边框 style 全是默认值，归一化分支从未被触发。用定向单测补上。
    #[test]
    fn normalizes_style_byte_80_to_zero() {
        assert_eq!(normalize_style(80), 0); // 'P' → 0
        assert_eq!(normalize_style(0), 0); // 0 与 80 归一后同值，但语义不同
        assert_eq!(normalize_style(42), 42); // 其余原样
        assert_eq!(normalize_style(-1), 255); // 负值按 u8 还原
    }

    /// 附件 B.3.1：位打包 short 默认 0x0020，仅 visible 置位。
    #[test]
    fn default_bit_flags_only_visible() {
        assert_eq!(bit::DEFAULT_FLAGS, bit::VISIBLE);
        assert_eq!(
            bit::DEFAULT_FLAGS & (bit::BOLD | bit::ITALIC | bit::UNDERLINE | bit::TEXT_WRAP),
            0
        );
    }

    /// 契约附件 B 的槽位数不得随重构漂移（RowCell 10 / ColCell 8 / NormalCell 49）。
    #[test]
    fn slot_counts_match_contract() {
        assert_eq!(row::SLOTS, 10);
        assert_eq!(col::SLOTS, 8);
        assert_eq!(cell::SLOTS, 49);
        assert_eq!(cell::ROTATION_ANGLE, cell::SLOTS); // 唯一 available() 守卫槽
        assert_eq!(def::CELL_TYPE, -64);
    }
}
