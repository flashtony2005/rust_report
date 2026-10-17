//! RowCell：行属性记录，10 槽（契约附件 B.1）。
//!
//! 槽序：rowType, rowHeight, rowVisible, breakPage, expMap(ByteMap 记录),
//! notes(readString), groupHeaderLevel(byte), OIS blob(int32 长度+跳过),
//! breakColumn, adjustRowH。
//! 证据：RowCell.java:350-376(写) / :381-402(读)。

#[cfg(feature = "legacy-rpx")]
use crate::error::Result;
use crate::props::def;
#[cfg(feature = "legacy-rpx")]
use crate::props::row;
use crate::reader::ObjValue;
#[cfg(feature = "legacy-rpx")]
use crate::reader::RecordReader;

#[derive(Debug, Clone, PartialEq)]
pub struct RowCell {
    pub row_type: i8,
    pub row_height: f32,
    pub row_visible: bool,
    pub break_page: bool,
    pub exp_map: Option<Vec<(u8, ObjValue)>>,
    pub notes: Option<String>,
    pub group_header_level: i8,
    /// 自定义属性 OIS blob（默认 6 字节 AC ED 00 05 70 70）
    pub ois_blob: Option<Vec<u8>>,
    pub break_column: bool,
    pub adjust_row_h: bool,
}

impl RowCell {
    /// 从 readRecord 切出的记录体解析；None = 空槽（size<=0）。
    #[cfg(feature = "legacy-rpx")]
    pub fn read(slice: &[u8]) -> Result<Option<Self>> {
        let mut r = RecordReader::new(slice);
        let row_type = r
            .read_i8()
            .map_err(|_| crate::error::RpxError::prop(row::ROW_TYPE as u16, "rowType"))?;
        let row_height = r
            .read_f32()
            .map_err(|_| crate::error::RpxError::prop(row::ROW_HEIGHT as u16, "rowHeight"))?;
        let row_visible = r
            .read_bool()
            .map_err(|_| crate::error::RpxError::prop(row::ROW_VISIBLE as u16, "rowVisible"))?;
        let break_page = r
            .read_bool()
            .map_err(|_| crate::error::RpxError::prop(row::BREAK_PAGE as u16, "breakPage"))?;
        let exp_map = match r
            .read_record_raw()
            .map_err(|_| crate::error::RpxError::prop(row::EXP_MAP as u16, "expMap"))?
        {
            Some(s) => Some(RecordReader::parse_byte_map(s)?),
            None => None,
        };
        let notes = r
            .read_string()
            .map_err(|_| crate::error::RpxError::prop(row::NOTES as u16, "notes"))?;
        let group_header_level = r
            .read_i8()
            .map_err(|_| crate::error::RpxError::prop(7, "groupHeaderLevel"))?;
        let ois_blob = r
            .read_blob()
            .map_err(|_| crate::error::RpxError::prop(row::OIS_BLOB as u16, "ois blob"))?;
        let break_column = r
            .read_bool()
            .map_err(|_| crate::error::RpxError::prop(row::BREAK_COLUMN as u16, "breakColumn"))?;
        let adjust_row_h = r
            .read_bool()
            .map_err(|_| crate::error::RpxError::prop(row::ADJUST_ROW_H as u16, "adjustRowH"))?;
        Ok(Some(Self {
            row_type,
            row_height,
            row_visible,
            break_page,
            exp_map,
            notes,
            group_header_level,
            ois_blob,
            break_column,
            adjust_row_h,
        }))
    }

    pub fn is_default(&self) -> bool {
        self.row_type == def::ROW_TYPE
            && (self.row_height - def::ROW_HEIGHT).abs() < 1e-5
            && self.row_visible
            && !self.break_page
            && self.exp_map.is_none()
            && self.notes.is_none()
            && self.group_header_level == 1
            && !self.break_column
            && !self.adjust_row_h
            && self.ois_blob.as_ref().map(|b| b.len()).unwrap_or(0) <= def::OIS_DEFAULT_LEN
    }
}
