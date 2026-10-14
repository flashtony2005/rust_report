//! ColCell：列属性记录，8 槽（契约附件 B.2）。
//!
//! 槽序：colType, colWidth, colVisible, breakPage, expMap(ByteMap 记录),
//! notes(readString), OIS blob, breakColumn。
//! 证据：ColCell.java:295-319(写) / :324-343(读)。

use crate::error::Result;
use crate::props::{col, def};
use crate::reader::{ObjValue, RecordReader};

#[derive(Debug, Clone, PartialEq)]
pub struct ColCell {
    pub col_type: i8,
    pub col_width: f32,
    pub col_visible: bool,
    pub break_page: bool,
    pub exp_map: Option<Vec<(u8, ObjValue)>>,
    pub notes: Option<String>,
    pub ois_blob: Option<Vec<u8>>,
    pub break_column: bool,
}

impl ColCell {
    pub fn read(slice: &[u8]) -> Result<Option<Self>> {
        let mut r = RecordReader::new(slice);
        let col_type = r
            .read_i8()
            .map_err(|_| crate::error::RpxError::prop(col::COL_TYPE as u16, "colType"))?;
        let col_width = r
            .read_f32()
            .map_err(|_| crate::error::RpxError::prop(col::COL_WIDTH as u16, "colWidth"))?;
        let col_visible = r
            .read_bool()
            .map_err(|_| crate::error::RpxError::prop(col::COL_VISIBLE as u16, "colVisible"))?;
        let break_page = r
            .read_bool()
            .map_err(|_| crate::error::RpxError::prop(col::BREAK_PAGE as u16, "breakPage"))?;
        let exp_map = match r
            .read_record_raw()
            .map_err(|_| crate::error::RpxError::prop(col::EXP_MAP as u16, "expMap"))?
        {
            Some(s) => Some(RecordReader::parse_byte_map(s)?),
            None => None,
        };
        let notes = r
            .read_string()
            .map_err(|_| crate::error::RpxError::prop(col::NOTES as u16, "notes"))?;
        let ois_blob = r
            .read_blob()
            .map_err(|_| crate::error::RpxError::prop(col::OIS_BLOB as u16, "ois blob"))?;
        let break_column = r
            .read_bool()
            .map_err(|_| crate::error::RpxError::prop(col::BREAK_COLUMN as u16, "breakColumn"))?;
        Ok(Some(Self {
            col_type,
            col_width,
            col_visible,
            break_page,
            exp_map,
            notes,
            ois_blob,
            break_column,
        }))
    }

    pub fn is_default(&self) -> bool {
        self.col_type == def::COL_TYPE
            && (self.col_width - def::COL_WIDTH).abs() < 1e-5
            && self.col_visible
            && !self.break_page
            && self.exp_map.is_none()
            && self.notes.is_none()
            && !self.break_column
            && self.ois_blob.as_ref().map(|b| b.len()).unwrap_or(0) <= def::OIS_DEFAULT_LEN
    }
}
