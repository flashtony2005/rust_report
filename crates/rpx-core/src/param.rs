//! Param：参数条目记录，10 槽（含 4 个读侧丢弃槽）。
//!
//! 槽序：paramType, dataType, pad(byte), name, description, null(String, 读侧丢弃),
//! value, short(丢弃), short(丢弃), null(String, 丢弃)。
//! ParamMetaData 外层 = int16 个数 + 逐条 readRecord(Param)（ParamMetaData.java:156-169）。
//! 证据：Param.java:113-126(写) / :129-141(读)。

use crate::error::{Result, RpxError};
use crate::reader::RecordReader;

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub param_type: i8,
    pub data_type: i8,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
}

impl Param {
    pub fn read(slice: &[u8]) -> Result<Self> {
        let mut r = RecordReader::new(slice);
        let p = |slot: u16, what: &'static str| RpxError::prop(slot, what);
        let param_type = r.read_i8().map_err(|_| p(1, "paramType"))?;
        let data_type = r.read_i8().map_err(|_| p(2, "dataType"))?;
        let _pad = r.read_i8().map_err(|_| p(3, "pad"))?;
        let name = r.read_string().map_err(|_| p(4, "name"))?;
        let description = r.read_string().map_err(|_| p(5, "description"))?;
        let _skip = r.read_string().map_err(|_| p(6, "skip"))?;
        let value = r.read_string().map_err(|_| p(7, "value"))?;
        let _s1 = r.read_i16().map_err(|_| p(8, "skip"))?;
        let _s2 = r.read_i16().map_err(|_| p(9, "skip"))?;
        let _s3 = r.read_string().map_err(|_| p(10, "skip"))?;
        Ok(Self {
            param_type,
            data_type,
            name,
            description,
            value,
        })
    }
}
