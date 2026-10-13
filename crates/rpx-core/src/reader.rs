//! 记录层读取器：ByteArrayInputRecord 的 Rust 对应物。
//!
//! 契约依据 docs/format-contract.md 附件 A：
//! - A.1 全大端原语（readBoolean 仅 ==1 为 true，EOF 报 Truncated）
//! - A.2 readString = int32 长度(-1=null) + 改良 UTF-8；readUTF = uint16 + 同体
//! - A.3 readStrings 个数是 int16，size<=0 → 整个数组 null
//! - A.4 readRecord = int32 长度 + 定长切片，size<=0 → 空槽（无标签字节）
//! - A.5 readObject = 1 字节标签字典（M1 子集）
//! - A.6 available() = 当前切片剩余字节数

use crate::error::{RpxError, Result};
use crate::javautf;

/// readObject 标签字典解出的值（M1 子集，附件 A.5）。
#[derive(Debug, Clone, PartialEq)]
pub enum ObjValue {
    Null,
    Str(String),
    Int(i32),
    Long(i64),
    Double(f64),
    Float(f32),
    Bool(bool),
    Byte(i8),
    Short(i16),
    /// 标签 2：int32 scale + 大端补码 unscaled 字节（M1 透传不转 decimal）
    BigDecimal { scale: i32, unscaled: Vec<u8> },
    /// 标签 7
    BigInteger(Vec<u8>),
    /// 标签 8
    Bytes(Vec<u8>),
    /// 标签 3/31/32/33：java.util.Date / sql.Timestamp / sql.Time / sql.Date，毫秒
    Millis { tag: i8, millis: i64 },
    /// 标签 0：IRecord 多态，M1 只留类名与原始字节
    Obj { class_name: Option<String>, raw: Vec<u8> },
}

/// 切片读取器。每个 readRecord 子记录用 `sub(slice)` 派生独立实例，
/// 因此 remaining() 只反映当前切片（附件 A.4/A.6）。
pub struct RecordReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> RecordReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub(crate) fn take(&mut self, n: usize, ctx: &'static str) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(RpxError::truncated(ctx));
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.take(1, "u8")?[0])
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_i16(&mut self) -> Result<i16> {
        let b = self.take(2, "i16")?;
        Ok(i16::from_be_bytes([b[0], b[1]]))
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let b = self.take(2, "u16")?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        let b = self.take(4, "i32")?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_i64(&mut self) -> Result<i64> {
        let b = self.take(8, "i64")?;
        Ok(i64::from_be_bytes(b.try_into().unwrap()))
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_i32()? as u32))
    }

    pub fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.read_i64()? as u64))
    }

    /// Java readBoolean：仅 1 为 true；字节不足报 Truncated（A.1 实现要点）。
    pub fn read_bool(&mut self) -> Result<bool> {
        self.read_u8().map(|b| b == 1)
    }

    /// A.2.1：int32 长度前缀，-1 → None；长度按字节数。
    pub fn read_string(&mut self) -> Result<Option<String>> {
        let len = self.read_i32()?;
        if len < 0 {
            return Ok(None);
        }
        let bytes = self.take(len as usize, "string body")?;
        Ok(Some(javautf::decode(bytes)?))
    }

    /// A.2.1：uint16 长度前缀，无 null 概念。
    pub fn read_utf(&mut self) -> Result<String> {
        let len = self.read_u16()? as usize;
        let bytes = self.take(len, "utf body")?;
        javautf::decode(bytes)
    }

    /// A.3：个数前缀是 int16；size<=0 → 整个数组为 None（只占 2 字节）。
    pub fn read_strings(&mut self) -> Result<Option<Vec<Option<String>>>> {
        let n = self.read_i16()?;
        if n <= 0 {
            return Ok(None);
        }
        let mut v = Vec::with_capacity(n as usize);
        for _ in 0..n {
            v.push(self.read_string()?);
        }
        Ok(Some(v))
    }

    /// A.4：int32 长度 + 定长切片；size<=0 → 空槽。返回的切片借自缓冲区。
    pub fn read_record_raw(&mut self) -> Result<Option<&'a [u8]>> {
        let size = self.read_i32()?;
        if size <= 0 {
            return Ok(None);
        }
        let s = self.take(size as usize, "record body")?;
        Ok(Some(s))
    }

    /// int32 长度 + 原始字节（OIS blob 等；len<=0 → None），返回拥有所有权的副本。
    pub fn read_blob(&mut self) -> Result<Option<Vec<u8>>> {
        Ok(self.read_record_raw()?.map(|s| s.to_vec()))
    }

    /// A.5：1 字节标签字典（M1 子集）。标签 -1=null，0=IRecord 多态，
    /// 1/13=String，2=BigDecimal，3/31/32/33=时间毫秒，4=Int，5=Long，6=Bool，
    /// 7=BigInteger，8=Bytes，9=Double，10=Float，11=Byte，12=Short。
    ///
    /// 未列出的标签按契约 A.5 末行**兜底 `readBytes()`**（int32 长度 + 字节）——与 Java
    /// 读侧默认分支一致（`ByteArrayInputRecord` 读侧 :263-265）。独立验证曾指出此处的
    /// 原实现返回错误、比契约严格，会拒掉 Java 能接受的文件，已按源码补齐（`UnknownTag`
    /// 错误变体随之失去构造点，已一并移除）。
    /// 注意取舍：兜底是宽松策略，未知标签不再显式报错（代价是这类输入的错误更晚暴露）。
    pub fn read_object(&mut self, ctx: &'static str) -> Result<ObjValue> {
        let tag = self.read_i8()?;
        let v = match tag {
            -1 => ObjValue::Null,
            0 => {
                let class_name = self.read_string()?;
                let raw = self.read_record_raw()?.unwrap_or(&[]).to_vec();
                ObjValue::Obj { class_name, raw }
            }
            1 => ObjValue::Str(self.read_utf()?),
            2 => {
                let scale = self.read_i32()?;
                let unscaled = self.read_blob()?.unwrap_or_default();
                ObjValue::BigDecimal { scale, unscaled }
            }
            3 | 31 | 32 | 33 => ObjValue::Millis { tag, millis: self.read_i64()? },
            4 => ObjValue::Int(self.read_i32()?),
            5 => ObjValue::Long(self.read_i64()?),
            6 => ObjValue::Bool(self.read_bool()?),
            7 => ObjValue::BigInteger(self.read_blob()?.unwrap_or_default()),
            8 => ObjValue::Bytes(self.read_blob()?.unwrap_or_default()),
            9 => ObjValue::Double(self.read_f64()?),
            10 => ObjValue::Float(self.read_f32()?),
            11 => ObjValue::Byte(self.read_i8()?),
            12 => ObjValue::Short(self.read_i16()?),
            13 => ObjValue::Str(self.read_string()?.unwrap_or_default()),
            _ => {
                let raw = self.read_blob().map_err(|_| RpxError::truncated(ctx))?;
                ObjValue::Bytes(raw.unwrap_or_default())
            }
        };
        Ok(v)
    }

    /// ByteMap 记录体解析（附件 B.3.3 第 4 条）：
    /// int16 count（大端）+ 逐条 byte key + readObject 值。
    /// 入参是 read_record_raw 切出的 ByteMap.serialize() 字节。
    pub fn parse_byte_map(slice: &[u8]) -> Result<Vec<(u8, ObjValue)>> {
        let mut r = RecordReader::new(slice);
        let count = r.read_i16()?;
        let mut out = Vec::new();
        if count > 0 {
            out.reserve(count as usize);
            for _ in 0..count {
                let key = r.read_u8()?;
                let val = r.read_object("byte_map entry")?;
                out.push((key, val));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{ObjValue, RecordReader};

    /// 契约附件 A.5 末行：未列出的标签兜底 `readBytes()`（int32 长度 + 字节），
    /// 与 Java 读侧默认分支一致 —— 不做严格拒绝，且必须保持流对齐。
    #[test]
    fn unknown_object_tag_falls_back_to_bytes() {
        let bytes = [42u8, 0, 0, 0, 3, 0xAA, 0xBB, 0xCC];
        let mut r = RecordReader::new(&bytes);
        match r.read_object("test").unwrap() {
            ObjValue::Bytes(b) => assert_eq!(b, vec![0xAA, 0xBB, 0xCC]),
            other => panic!("expected Bytes fallback, got {:?}", other),
        }
        assert_eq!(r.remaining(), 0, "fallback must consume exactly length+body");
    }

    /// 兜底路径的 size<=0（含 -1 null）→ 空字节，且只吃掉 4 字节长度前缀，不越界吞后续。
    #[test]
    fn fallback_blob_with_non_positive_length_is_empty() {
        let mut body = vec![42u8];
        body.extend_from_slice(&(-1i32).to_be_bytes());
        body.push(0x7F); // 后续字节：兜底不得越界吞掉
        let mut r = RecordReader::new(&body);
        match r.read_object("test").unwrap() {
            ObjValue::Bytes(b) => assert!(b.is_empty()),
            other => panic!("got {:?}", other),
        }
        assert_eq!(r.remaining(), 1, "must not consume past the length prefix");
    }

    /// 兜底路径同样受切片边界保护：长度声明超出剩余字节 → Truncated。
    #[test]
    fn fallback_blob_truncation_is_error() {
        let bytes = [42u8, 0, 0, 0, 9, 0x01]; // 声明 9 字节，实际只有 1
        let mut r = RecordReader::new(&bytes);
        assert!(r.read_object("test").is_err());
    }
}
