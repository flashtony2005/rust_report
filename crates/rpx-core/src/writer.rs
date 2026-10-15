//! 记录层写入器 + 文件层封装（`.rpx` **写侧**，M7-2）。
//!
//! 与 [`crate::reader`] 逐条互逆，契约依据 `docs/format-contract.md` 附件 A：
//! - A.1 全大端原语；`writeBoolean` = 1/0 单字节（读侧仅 1 为 true）；
//! - A.2.1 `writeString` = int32 长度（**-1 = null**）+ 改良 UTF-8；`writeUTF` = uint16 + 同体；
//! - A.3 `writeStrings(null)` = **int16 0**（整个数组 null 只占 2 字节）；
//! - A.4 `writeRecord(null)` = **int32 -1**（4 字节）；有值 = int32 长度 + 字节；
//! - A.5 `writeObject` = 1 字节标签字典（与读侧同表）。
//!
//! # 两处「读侧丢信息 ⇒ 写侧只能取规范形态」的取舍（诚实声明）
//!
//! 1. **字符串标签**：读侧标签 `1`（uint16 前缀）与 `13`（int32 前缀）都解成
//!    [`ObjValue::Str`]，**标签本身被丢掉**。写侧按 `NormalCell.java:1493` 的
//!    原始规则还原：编码后 ≤65535 字节走标签 `1`，否则走标签 `13` —— 这正是
//!    真实 writer 的行为，故对真实文件**可字节复现**（>65535 字节的字符串在
//!    报表里不存在，但规则本身是对的）。
//! 2. **空 blob 的两种形态**：Java 侧 `writeBytes(null)` = -1、`writeBytes(new byte[0])` = 0，
//!    而读侧 [`crate::reader::RecordReader::read_blob`] 把 `size <= 0` 一律并成 `None`。
//!    写侧对空/缺省一律写 **-1**（模型无法区分二者，取读侧能还原的那一种）。
//!
//! 往返判据（逐夹具字节级）在 `crates/rpx-core/tests/roundtrip.rs`。

use crate::error::{Result, RpxError};
use crate::javautf;
use crate::reader::ObjValue;
use crate::report::ReportDefine;

mod payload;
mod records;

pub use payload::write_payload;

/// `writeUTF` 的长度上界（uint16，附件 A.2.1）。
pub const UTF_MAX: usize = 65_535;

/// 记录层写入器：累积字节，提供与 [`crate::reader::RecordReader`] 逐条互逆的原语。
#[derive(Default)]
pub struct RecordWriter {
    buf: Vec<u8>,
}

impl RecordWriter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// 已写字节数（与读侧 `remaining()` 对偶）。
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// 取出字节（消费写入器）。
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// 借用当前字节（用于「子记录写完再包长度」的装配）。
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn write_i8(&mut self, v: i8) {
        self.buf.push(v as u8);
    }

    /// `writeBoolean`：true → 1、false → 0（附件 A.1）。
    pub fn write_bool(&mut self, v: bool) {
        self.buf.push(u8::from(v));
    }

    pub fn write_i16(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_bits().to_be_bytes());
    }

    pub fn write_f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_bits().to_be_bytes());
    }

    /// `writeString`：int32 长度（-1 = null）+ 改良 UTF-8（附件 A.2.1）。
    pub fn write_string(&mut self, s: Option<&str>) {
        match s {
            None => self.write_i32(-1),
            Some(s) => self.write_len_prefixed(&javautf::encode(s)),
        }
    }

    /// `writeUTF`：uint16 长度 + 改良 UTF-8；编码后超 65535 字节报错
    /// （调用方应改走 [`Self::write_string`]，见模块文档取舍 1）。
    pub fn write_utf(&mut self, s: &str) -> Result<()> {
        let bytes = javautf::encode(s);
        if bytes.len() > UTF_MAX {
            return Err(RpxError::Utf8 {
                detail: "writeUTF: encoded length exceeds 65535",
            });
        }
        self.write_i16(bytes.len() as i16);
        self.buf.extend_from_slice(&bytes);
        Ok(())
    }

    /// `writeStrings`：int16 个数；`None` 与空数组都写 2 字节 `0`（附件 A.3）。
    pub fn write_strings(&mut self, v: Option<&[Option<String>]>) {
        let items = v.unwrap_or(&[]);
        self.write_i16(items.len() as i16);
        for it in items {
            self.write_string(it.as_deref());
        }
    }

    /// `writeRecord`：`None` → int32 **-1**；有值 → int32 长度 + 字节（附件 A.4）。
    pub fn write_record(&mut self, body: Option<&[u8]>) {
        match body {
            None => self.write_i32(-1),
            Some(b) => self.write_len_prefixed(b),
        }
    }

    /// `writeBytes` 的模型口径：`None` 与空字节都写 **-1**（模块文档取舍 2）。
    pub fn write_blob(&mut self, b: Option<&[u8]>) {
        match b {
            Some(b) if !b.is_empty() => self.write_len_prefixed(b),
            _ => self.write_i32(-1),
        }
    }

    /// `writeObject`：1 字节标签 + 载荷（附件 A.5，与读侧同表）。
    pub fn write_object(&mut self, v: &ObjValue) -> Result<()> {
        match v {
            ObjValue::Null => self.write_i8(-1),
            ObjValue::Str(s) => {
                // ≤65535 走标签 1 + writeUTF，否则标签 13 + writeString（见模块文档取舍 1）。
                if javautf::encode(s).len() <= UTF_MAX {
                    self.write_i8(1);
                    self.write_utf(s)?;
                } else {
                    self.write_i8(13);
                    self.write_string(Some(s));
                }
            }
            ObjValue::BigDecimal { scale, unscaled } => {
                self.write_i8(2);
                self.write_i32(*scale);
                self.write_blob(Some(unscaled));
            }
            ObjValue::Millis { tag, millis } => {
                self.write_i8(*tag);
                self.write_i64(*millis);
            }
            ObjValue::Int(n) => {
                self.write_i8(4);
                self.write_i32(*n);
            }
            ObjValue::Long(n) => {
                self.write_i8(5);
                self.write_i64(*n);
            }
            ObjValue::Bool(b) => {
                self.write_i8(6);
                self.write_bool(*b);
            }
            ObjValue::BigInteger(b) => {
                self.write_i8(7);
                self.write_blob(Some(b));
            }
            ObjValue::Bytes(b) => {
                self.write_i8(8);
                self.write_blob(Some(b));
            }
            ObjValue::Double(f) => {
                self.write_i8(9);
                self.write_f64(*f);
            }
            ObjValue::Float(f) => {
                self.write_i8(10);
                self.write_f32(*f);
            }
            ObjValue::Byte(n) => {
                self.write_i8(11);
                self.write_i8(*n);
            }
            ObjValue::Short(n) => {
                self.write_i8(12);
                self.write_i16(*n);
            }
            ObjValue::Obj { class_name, raw } => {
                self.write_i8(0);
                self.write_string(class_name.as_deref());
                self.write_record(Some(raw));
            }
        }
        Ok(())
    }

    /// `ByteMap` 记录体：int16 count + 逐条 `byte key` + `writeObject(值)`（附件 B.3.3 第 4 条）。
    pub fn write_byte_map(&mut self, entries: &[(u8, ObjValue)]) -> Result<()> {
        self.write_i16(entries.len() as i16);
        for (k, v) in entries {
            self.write_u8(*k);
            self.write_object(v)?;
        }
        Ok(())
    }

    /// int32 长度前缀 + 字节（`writeString`/`writeRecord`/`writeBytes` 的共同尾部）。
    fn write_len_prefixed(&mut self, bytes: &[u8]) {
        self.write_i32(bytes.len() as i32);
        self.buf.extend_from_slice(bytes);
    }
}

/// 装配「记录体」：把闭包写进一个子写入器，再取字节。
///
/// 本仓的记录是**长度前缀包裹**的（附件 A.4），所以每写一个子记录都要先拿到
/// 它的完整字节才能写长度 —— 这个 helper 把那对「新建 → 写 → 取字节」收敛到一处。
pub(crate) fn record_body<F>(f: F) -> Result<Vec<u8>>
where
    F: FnOnce(&mut RecordWriter) -> Result<()>,
{
    let mut w = RecordWriter::new();
    f(&mut w)?;
    Ok(w.into_bytes())
}

/// [`ReportDefine`] → `.rpx` 文件字节（文件层契约：`docs/format-contract.md` 附件 C.5）。
///
/// `[RQQR 4B][int32 BE payload 长度 N][payload N B][页脚 8B]`，页脚 = 魔数两遍。
pub fn to_bytes(rd: &ReportDefine) -> Result<Vec<u8>> {
    let payload = payload::write_payload(rd)?;
    let mut out = Vec::with_capacity(payload.len() + 16);
    out.extend_from_slice(&crate::MAGIC);
    out.extend_from_slice(&(payload.len() as i32).to_be_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&crate::FOOTER);
    Ok(out)
}

#[cfg(test)]
mod tests;
