//! 改良 UTF-8（Java DataOutputStream.writeUTF 语义）解码器。
//!
//! 契约依据 docs/format-contract.md 附件 A.2.2：
//! - 1 字节：0x0001-0x007F
//! - 2 字节：0x0080-0x07FF；`\0` 编码为 0xC0 0x80
//! - 3 字节：0x0800-0xFFFF（代理对的一半各 3 字节，合计 6 字节）
//! - 解码宽容度：接受单字节 0x00（写侧不产此形态，属解码器超集）
//! - 首字节 0x80-0xBF / 0xF0-0xFF → 错误

use crate::error::{RpxError, Result};

/// 解码改良 UTF-8 字节序列为 Rust String。
/// 代理对按 Java UTF-16 语义组合为同一码点；未配对代理替换为 U+FFFD。
pub fn decode(bytes: &[u8]) -> Result<String> {
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        let unit = if c < 0x80 {
            i += 1;
            c as u16
        } else if (0xC0..=0xDF).contains(&c) {
            if i + 1 >= bytes.len() {
                return Err(RpxError::Utf8 { detail: "2-byte sequence truncated" });
            }
            let b2 = bytes[i + 1];
            if b2 & 0xC0 != 0x80 {
                return Err(RpxError::Utf8 { detail: "bad continuation byte (2-byte)" });
            }
            i += 2;
            (((c & 0x1F) as u16) << 6) | ((b2 & 0x3F) as u16)
        } else if (0xE0..=0xEF).contains(&c) {
            if i + 2 >= bytes.len() {
                return Err(RpxError::Utf8 { detail: "3-byte sequence truncated" });
            }
            let b2 = bytes[i + 1];
            let b3 = bytes[i + 2];
            if b2 & 0xC0 != 0x80 || b3 & 0xC0 != 0x80 {
                return Err(RpxError::Utf8 { detail: "bad continuation byte (3-byte)" });
            }
            i += 3;
            (((c & 0x0F) as u16) << 12) | (((b2 & 0x3F) as u16) << 6) | ((b3 & 0x3F) as u16)
        } else {
            return Err(RpxError::Utf8 { detail: "invalid leading byte" });
        };
        units.push(unit);
    }
    Ok(String::from_utf16_lossy(&units))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_ascii() {
        assert_eq!(decode(b"ReportLite").unwrap(), "ReportLite");
    }

    #[test]
    fn decodes_nul_as_two_bytes() {
        // 写侧把 '\0' 编成 0xC0 0x80
        assert_eq!(decode(&[0xC0, 0x80]).unwrap(), "\0");
    }

    #[test]
    fn decodes_two_and_three_byte() {
        // U+0080 -> C2 80 ; U+4E2D(中) -> E4 B8 AD
        assert_eq!(decode(&[0xC2, 0x80]).unwrap(), "\u{80}");
        assert_eq!(decode(&[0xE4, 0xB8, 0xAD]).unwrap(), "\u{4e2d}");
    }

    #[test]
    fn decodes_surrogate_pair() {
        // U+1F600 代理对 D83D DE00 -> ED A0 BD ED B8 80
        assert_eq!(decode(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]).unwrap(), "\u{1f600}");
    }

    #[test]
    fn rejects_bad_leading_byte() {
        assert!(decode(&[0xF0, 0x80]).is_err());
    }
}
