//! NormalCell：普通单元格记录，49 槽（契约附件 B.3.2）。
//!
//! 五个必守陷阱（附件 B.3.2/B.3.3）：
//! - 槽 4/24/27/30/33/43 六个 style 字节读侧 80→0 归一化；
//! - 槽 13 cellObj 存在标志，true 时类名 + 记录原始字节（M1 不实例化）；
//! - 槽 36 expMap 是 ByteMap 记录（int16 count + 条目）；
//! - 槽 40 是 writeStrings(null) 钉死的 2 字节空洞（读侧丢弃）；
//! - 槽 41 OIS blob 按 int32 长度跳过并原样保留（默认 6 字节）；
//! - 槽 49 rotationAngle 是全模型唯一 available() 守卫槽。
//!
//! 证据：NormalCell.java:1468-1539(写) / :1544-1616(读)。
//!
//! 位标志访问器与默认值判定在 `cell_defaults` 子模块。

pub mod cell_defaults;

#[cfg(feature = "legacy-rpx")]
use crate::error::{Result, RpxError};
#[cfg(feature = "legacy-rpx")]
use crate::props::{cell, def, normalize_style};
use crate::reader::ObjValue;
#[cfg(feature = "legacy-rpx")]
use crate::reader::RecordReader;

/// 合并区/扩展区：4×int32 大端，流内顺序 r1(beginRow), r2(endRow), c1(beginCol), c2(endCol)。
/// 证据：Area.java:126-142。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub begin_row: i32,
    pub end_row: i32,
    pub begin_col: i32,
    pub end_col: i32,
}

impl Area {
    #[cfg(feature = "legacy-rpx")]
    fn read(slot: usize) -> impl FnOnce(&[u8]) -> Result<Option<Area>> {
        move |slice: &[u8]| {
            let mut r = RecordReader::new(slice);
            let begin_row = r
                .read_i32()
                .map_err(|_| RpxError::prop(slot as u16, "area beginRow"))?;
            let end_row = r
                .read_i32()
                .map_err(|_| RpxError::prop(slot as u16, "area endRow"))?;
            let begin_col = r
                .read_i32()
                .map_err(|_| RpxError::prop(slot as u16, "area beginCol"))?;
            let end_col = r
                .read_i32()
                .map_err(|_| RpxError::prop(slot as u16, "area endCol"))?;
            Ok(Some(Area {
                begin_row,
                end_row,
                begin_col,
                end_col,
            }))
        }
    }
}

/// 槽 13 cellObj：多态对象引用（M1 保留类名与原始字节）。
#[derive(Debug, Clone, PartialEq)]
pub struct CellObj {
    pub class_name: Option<String>,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub merged_area: Option<Area>,
    pub h_align: i8,
    pub v_align: i8,
    pub diagonal_style: u8,
    pub diagonal_color: i32,
    pub diagonal_width: f32,
    pub extend_mode: i8,
    pub extend_area: Option<Area>,
    pub left_head: Option<String>,
    pub top_head: Option<String>,
    pub to_excel_mode: i8,
    pub adjust_size_mode: i8,
    pub cell_obj: Option<CellObj>,
    pub value: ObjValue,
    pub disp_value: Option<String>,
    pub indent: f32,
    pub font_name: Option<String>,
    pub font_size: i16,
    pub format: Option<String>,
    pub fore_color: i32,
    pub back_color: i32,
    pub hyperlink: Option<String>,
    pub hyperlink_window: Option<String>,
    pub lb_style: u8,
    pub lb_width: f32,
    pub lb_color: i32,
    pub rb_style: u8,
    pub rb_width: f32,
    pub rb_color: i32,
    pub tb_style: u8,
    pub tb_width: f32,
    pub tb_color: i32,
    pub bb_style: u8,
    pub bb_width: f32,
    pub bb_color: i32,
    pub exp_map: Option<Vec<(u8, ObjValue)>>,
    pub bit_flags: u16,
    pub cell_type: i8,
    pub notes: Option<String>,
    pub ois_blob: Option<Vec<u8>>,
    pub cell_graph_config: Option<Vec<u8>>,
    pub under_line_style: u8,
    pub cell_style_name: Option<String>,
    pub merge_same: i8,
    pub merge_null: i8,
    pub merge_same_mode: i8,
    pub merge_null_mode: i8,
    pub rotation_angle: f32,
}

impl Cell {
    /// 从 readRecord 切出的记录体解析；None = 空槽（合并非锚点 = int32 -1）。
    #[cfg(feature = "legacy-rpx")]
    pub fn read(slice: &[u8]) -> Result<Option<Self>> {
        let mut r = RecordReader::new(slice);
        let p = |slot: usize, what: &'static str| RpxError::prop(slot as u16, what);

        let merged_area = match r
            .read_record_raw()
            .map_err(|_| p(cell::MERGED_AREA, "mergedArea"))?
        {
            Some(s) => Area::read(cell::MERGED_AREA)(s)?,
            None => None,
        };
        let h_align = r.read_i8().map_err(|_| p(cell::HALIGN, "hAlign"))?;
        let v_align = r.read_i8().map_err(|_| p(cell::VALIGN, "vAlign"))?;
        let diagonal_style = normalize_style(
            r.read_i8()
                .map_err(|_| p(cell::DIAGONAL_STYLE, "diagonalStyle"))?,
        );
        let diagonal_color = r
            .read_i32()
            .map_err(|_| p(cell::DIAGONAL_COLOR, "diagonalColor"))?;
        let diagonal_width = r
            .read_f32()
            .map_err(|_| p(cell::DIAGONAL_WIDTH, "diagonalWidth"))?;
        let extend_mode = r
            .read_i8()
            .map_err(|_| p(cell::EXTEND_MODE, "extendMode"))?;
        let extend_area = match r
            .read_record_raw()
            .map_err(|_| p(cell::EXTEND_AREA, "extendArea"))?
        {
            Some(s) => Area::read(cell::EXTEND_AREA)(s)?,
            None => None,
        };
        let left_head = r
            .read_string()
            .map_err(|_| p(cell::LEFT_HEAD, "leftHead"))?;
        let top_head = r.read_string().map_err(|_| p(cell::TOP_HEAD, "topHead"))?;
        let to_excel_mode = r
            .read_i8()
            .map_err(|_| p(cell::TO_EXCEL_MODE, "toExcelMode"))?;
        let adjust_size_mode = r
            .read_i8()
            .map_err(|_| p(cell::ADJUST_SIZE_MODE, "adjustSizeMode"))?;

        let cell_obj = if r
            .read_bool()
            .map_err(|_| p(cell::CELL_OBJ, "cellObj flag"))?
        {
            let class_name = r
                .read_string()
                .map_err(|_| p(cell::CELL_OBJ, "cellObj class"))?;
            let raw = r
                .read_record_raw()
                .map_err(|_| p(cell::CELL_OBJ, "cellObj record"))?
                .unwrap_or(&[])
                .to_vec();
            Some(CellObj { class_name, raw })
        } else {
            None
        };

        let value = r
            .read_object("cell VALUE")
            .map_err(|_| p(cell::VALUE, "value"))?;
        let disp_value = r
            .read_string()
            .map_err(|_| p(cell::DISP_VALUE, "dispValue"))?;
        let indent = r.read_f32().map_err(|_| p(cell::INDENT, "indent"))?;
        let font_name = r
            .read_string()
            .map_err(|_| p(cell::FONT_NAME, "fontName"))?;
        let font_size = r.read_i16().map_err(|_| p(cell::FONT_SIZE, "fontSize"))?;
        let format = r.read_string().map_err(|_| p(cell::FORMAT, "format"))?;
        let fore_color = r.read_i32().map_err(|_| p(cell::FORE_COLOR, "foreColor"))?;
        let back_color = r.read_i32().map_err(|_| p(cell::BACK_COLOR, "backColor"))?;
        let hyperlink = r
            .read_string()
            .map_err(|_| p(cell::HYPERLINK, "hyperlink"))?;
        let hyperlink_window = r
            .read_string()
            .map_err(|_| p(cell::HYPERLINK_WINDOW, "hyperlinkWindow"))?;
        let lb_style = normalize_style(r.read_i8().map_err(|_| p(cell::LB_STYLE, "lbStyle"))?);
        let lb_width = r.read_f32().map_err(|_| p(cell::LB_WIDTH, "lbWidth"))?;
        let lb_color = r.read_i32().map_err(|_| p(cell::LB_COLOR, "lbColor"))?;
        let rb_style = normalize_style(r.read_i8().map_err(|_| p(cell::RB_STYLE, "rbStyle"))?);
        let rb_width = r.read_f32().map_err(|_| p(cell::RB_WIDTH, "rbWidth"))?;
        let rb_color = r.read_i32().map_err(|_| p(cell::RB_COLOR, "rbColor"))?;
        let tb_style = normalize_style(r.read_i8().map_err(|_| p(cell::TB_STYLE, "tbStyle"))?);
        let tb_width = r.read_f32().map_err(|_| p(cell::TB_WIDTH, "tbWidth"))?;
        let tb_color = r.read_i32().map_err(|_| p(cell::TB_COLOR, "tbColor"))?;
        let bb_style = normalize_style(r.read_i8().map_err(|_| p(cell::BB_STYLE, "bbStyle"))?);
        let bb_width = r.read_f32().map_err(|_| p(cell::BB_WIDTH, "bbWidth"))?;
        let bb_color = r.read_i32().map_err(|_| p(cell::BB_COLOR, "bbColor"))?;

        let exp_map = match r
            .read_record_raw()
            .map_err(|_| p(cell::EXP_MAP, "expMap"))?
        {
            Some(s) => Some(RecordReader::parse_byte_map(s)?),
            None => None,
        };
        let bit_flags = r.read_u16().map_err(|_| p(cell::BIT_FLAGS, "bitFlags"))?;
        let cell_type = r.read_i8().map_err(|_| p(cell::CELL_TYPE, "cellType"))?;
        let notes = r.read_string().map_err(|_| p(cell::NOTES, "notes"))?;

        // 槽 40：writeStrings(null) 的 2 字节空洞，读侧丢弃（附件 A.3）
        let _hole = r
            .read_strings()
            .map_err(|_| p(cell::STRINGS_HOLE, "strings hole"))?;

        let ois_blob = r.read_blob().map_err(|_| p(cell::OIS_BLOB, "ois blob"))?;
        let cell_graph_config = r
            .read_blob()
            .map_err(|_| p(cell::CELL_GRAPH_CONFIG, "cellGraphConfig"))?;
        let under_line_style = normalize_style(
            r.read_i8()
                .map_err(|_| p(cell::UNDER_LINE_STYLE, "underLineStyle"))?,
        );
        let cell_style_name = r
            .read_string()
            .map_err(|_| p(cell::CELL_STYLE_NAME, "cellStyleName"))?;
        let merge_same = r.read_i8().map_err(|_| p(cell::MERGE_SAME, "mergeSame"))?;
        let merge_null = r.read_i8().map_err(|_| p(cell::MERGE_NULL, "mergeNull"))?;
        let merge_same_mode = r
            .read_i8()
            .map_err(|_| p(cell::MERGE_SAME_MODE, "mergeSameMode"))?;
        let merge_null_mode = r
            .read_i8()
            .map_err(|_| p(cell::MERGE_NULL_MODE, "mergeNullMode"))?;

        // 槽 49：唯一 available() 守卫槽（NormalCell.java:1613-1615）
        let rotation_angle = if r.remaining() > 0 {
            r.read_f32()
                .map_err(|_| p(cell::ROTATION_ANGLE, "rotationAngle"))?
        } else {
            def::ROTATION_ANGLE
        };

        Ok(Some(Self {
            merged_area,
            h_align,
            v_align,
            diagonal_style,
            diagonal_color,
            diagonal_width,
            extend_mode,
            extend_area,
            left_head,
            top_head,
            to_excel_mode,
            adjust_size_mode,
            cell_obj,
            value,
            disp_value,
            indent,
            font_name,
            font_size,
            format,
            fore_color,
            back_color,
            hyperlink,
            hyperlink_window,
            lb_style,
            lb_width,
            lb_color,
            rb_style,
            rb_width,
            rb_color,
            tb_style,
            tb_width,
            tb_color,
            bb_style,
            bb_width,
            bb_color,
            exp_map,
            bit_flags,
            cell_type,
            notes,
            ois_blob,
            cell_graph_config,
            under_line_style,
            cell_style_name,
            merge_same,
            merge_null,
            merge_same_mode,
            merge_null_mode,
            rotation_angle,
        }))
    }
}
