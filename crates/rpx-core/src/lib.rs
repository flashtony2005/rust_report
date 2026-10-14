//! rpx-core：ReportLite .rpx 二进制格式解析器（纯 std，零依赖）。
//!
//! 文件层契约（ReportUtils.java:70-96 写 / :27-53 读）：
//! `[RQQR 4B][int32 BE payload 长度 N][payload N B][页脚 8B]`
//! - 魔数不命中 → UnsupportedFormat（含 Java OIS 老格式，AC-01/AC-09 不回退）；
//! - 页脚按 AC 校验（真实 writer 必写，夹具必然带）；
//! - payload 即 ReportDefine.serialize() 字节，按附件 C.2 的 24 步装载。

pub mod cell;
pub mod colcell;
pub mod dataset;
pub mod error;
pub mod javautf;
pub mod param;
pub mod print_setup;
pub mod props;
pub mod reader;
pub mod report;
pub mod rowcell;
pub mod rowset;
pub mod value;

pub use error::{Result, RpxError};
pub use report::{CellId, MergedArea, ReportDefine, ScalarTail};

const MAGIC: [u8; 4] = [82, 81, 81, 82];
const FOOTER: [u8; 8] = [82, 81, 81, 82, 82, 81, 81, 82];

/// 主入口：从任意 Read 源解析 .rpx。
pub fn open<R: std::io::Read>(mut r: R) -> Result<ReportDefine> {
    let mut buf = Vec::new();
    r.read_to_end(&mut buf)?;
    open_bytes(&buf)
}

/// 主入口（字节版）：校验文件层后装载 payload。
pub fn open_bytes(buf: &[u8]) -> Result<ReportDefine> {
    if buf.len() < 8 {
        return Err(RpxError::Truncated { ctx: "file header" });
    }
    // AC-01/AC-09：魔数不命中即不支持（不尝试 OIS 回退）
    if buf[0..4] != MAGIC {
        return Err(RpxError::UnsupportedFormat);
    }
    let declared = i64::from(i32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]));
    if declared < 0 {
        return Err(RpxError::BadLength {
            declared,
            file_len: buf.len(),
        });
    }
    let payload_end = 8 + declared as usize;
    // AC-02：payload 实际字节不足
    if payload_end > buf.len() {
        return Err(RpxError::Truncated { ctx: "payload" });
    }
    // 页脚校验（AC：真实 writer 必写 8 字节页脚）
    if payload_end + 8 > buf.len() || buf[payload_end..payload_end + 8] != FOOTER {
        return Err(RpxError::BadFooter);
    }
    ReportDefine::fill(&buf[8..payload_end])
}
