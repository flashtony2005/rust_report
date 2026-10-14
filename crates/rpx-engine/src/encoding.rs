//! 数据集文件解码：`FileDataSetConfig.encoding`（默认 `"GBK"`）→ UTF-8 `String`。
//!
//! # 为什么必须自己做（零依赖约束）
//!
//! `rpx-engine` 与 `rpx-core` 同属**零第三方依赖层**（G5：`scripts/gate.sh`），
//! 不能引 `encoding_rs`。GBK 映射表沿用 M2-12 collation 的先例（`collation.bin`）：
//! **冻结二进制表 + `include_bytes!`**，零运行时文件依赖。
//!
//! # 数据链（表的权威来源 = 产品真机，不是第三方标准表）
//!
//! ```text
//! 产品：FileDataSetFactory.java:112  new FileObject(fileName, cs, "s", ctx)   ← cs 默认 "GBK"（:25）
//!   → JDK GBK charset（sun.nio.cs.ext.GBK）
//!   → ProbeEncoding.java 全定义域 dump（tests/fixtures/gbk_jdk.bin，23940 对）
//!   → scripts/gen_gbk_table.py（只校验+加头+排序，不发明映射）
//!   → data/gbk.bin（本模块 include_bytes!）
//! ```
//!
//! 即：**表里的每一对映射都是 JDK 的真值**（含 JDK 私有区映射，故比常见标准表
//! 的 21886 项多 —— 23940 项正是"以产品为准"与"以标准表为准"的差别）。
//!
//! # 支持范围（诚实声明）
//!
//! * **GBK 双字节**（lead 0x81-0xFE × trail 0x40-0xFE 除 0x7F）+ **ASCII 单字节**；
//! * 显式声明 `UTF-8`/`UTF8` 的模板走 `std::str::from_utf8`（兼容既有夹具）；
//! * **GB18030 四字节序列暂不支持**：产品默认 `encoding="GBK"` 时 JDK 的 GBK
//!   charset 同样不接受四字节（`MalformedInputException`）；模板显式写
//!   `GB18030` 且文件含四字节序列的场景**无夹具**，按"禁止无基准对拍"带账
//!   （`spec-m3.md` §5），命中时报 [`EngineError::UnsupportedEncoding`]。
//! * 其它 charset 名（`Big5`/`UTF-16`…）一律 [`EngineError::UnsupportedEncoding`]。
//!
//! # 非法字节：为什么显式报错而不是 U+FFFD 替换
//!
//! JDK charset 的默认 `CodingErrorAction` 是 **REPLACE**（非法字节 → U+FFFD，
//! 不报错）。本实现选择**显式失败**：M3-02 要消灭的失败模式正是"静默乱码"——
//! CSV 是产品资产，解码层把非法字节换成 U+FFFD 会让错误的数据悄悄流进
//! 计算态。这是与产品行为的一处**有意偏离**，记录在此（夹具内容恒为合法
//! GBK，真实链路不会触发）。

use crate::error::{EngineError, Result};

/// 冻结的 JDK GBK 映射表（`scripts/gen_gbk_table.py` 从 `ProbeEncoding.java`
/// 的 dump 生成；格式 = 8 字节 magic + u32 BE 条数 + 条数 × (u16 BE, u16 BE)）。
static GBK_TABLE: &[u8] = include_bytes!("../data/gbk.bin");

/// magic `b"RPXGBK\x01\x00"` + u32 BE count 的头部长度。
const HEADER_LEN: usize = 12;

/// 按 charset 名解码数据集文件字节 → UTF-8 `String`。
///
/// `None` 或缺省按产品默认 `"GBK"`（`FileDataSetConfig.java:25`）——这是**产品
/// 口径**而非本 crate 的选择；`writeCsv`（`CalcProbe.java:277`）也一直按 GBK 写。
/// charset 名大小写不敏感（与 `Charset.forName` 一致）。
pub fn decode(bytes: &[u8], encoding: Option<&str>) -> Result<String> {
    let name = encoding.map(str::trim).unwrap_or("");
    // 大小写不敏感比较（Charset.forName 语义）；空串等同未声明。
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "" | "GBK" | "CP936" | "MS936" | "WINDOWS-936" => decode_gbk(bytes),
        "UTF-8" | "UTF8" => std::str::from_utf8(bytes).map(str::to_string).map_err(|e| {
            EngineError::MalformedEncoding {
                detail: format!("UTF-8 解码失败：{e}"),
            }
        }),
        other => Err(EngineError::UnsupportedEncoding {
            name: other.to_string(),
        }),
    }
}

/// GBK 解码（ASCII 单字节直通；双字节查冻结表）。
///
/// 非法输入（孤立高字节 / 表外双字节序列）→ [`EngineError::MalformedEncoding`]。
pub fn decode_gbk(bytes: &[u8]) -> Result<String> {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            // ASCII：GBK 单字节区与 ASCII 逐字节一致（探针锚点已覆盖）。
            // push char 而非 push byte：避免对 b>=0x80 分支的误用。
            out.push(b as char);
            i += 1;
            continue;
        }
        // 双字节：lead 必须在 0x81-0xFE，否则孤立高字节。
        if !(0x81..=0xFE).contains(&b) {
            return Err(malformed(i, "孤立高字节（GBK 双字节序列不完整）"));
        }
        let Some(&trail) = bytes.get(i + 1) else {
            return Err(malformed(i, "文件以 GBK 前导字节结尾（序列不完整）"));
        };
        // trail ∈ [0x40,0xFE] 且 != 0x7F（0x7F 在 GBK 里是保留位）。
        if !(0x40..=0xFE).contains(&trail) || trail == 0x7F {
            return Err(malformed(i, "GBK 尾字节越界"));
        }
        let code = (u16::from(b) << 8) | u16::from(trail);
        match lookup(code) {
            Some(uni) => {
                // 表由 JDK dump 冻结：映射目标恒为 BMP 非代理区标量
                // （生成器校验过 D800-DFFF 排除），char::from_u32 不会失败。
                let ch =
                    char::from_u32(u32::from(uni)).expect("gbk.bin 含代理区（生成器应已排除）");
                out.push(ch);
            }
            None => return Err(malformed(i, "GBK 码位未定义（表外序列）")),
        }
        i += 2;
    }
    Ok(out)
}

/// 冻结表二分查找：GBK 双字节码位 → BMP Unicode 标量。
fn lookup(code: u16) -> Option<u16> {
    let body = GBK_TABLE.get(HEADER_LEN..)?;
    let count = u32::from_be_bytes([
        *GBK_TABLE.get(8)?,
        *GBK_TABLE.get(9)?,
        *GBK_TABLE.get(10)?,
        *GBK_TABLE.get(11)?,
    ]) as usize;
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let off = mid * 4;
        let g = u16::from_be_bytes([body[off], body[off + 1]]);
        if g == code {
            return Some(u16::from_be_bytes([body[off + 2], body[off + 3]]));
        }
        if g < code {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    None
}

/// 统一构造 MalformedEncoding（带字节偏移，便于定位坏数据）。
fn malformed(offset: usize, what: &str) -> EngineError {
    EngineError::MalformedEncoding {
        detail: format!("偏移 {offset} 处：{what}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_frozen_and_lookup_works() {
        // 锚点与探针输出逐一对拍（ProbeEncoding.java 的 GBK_ANCHORS）。
        assert_eq!(lookup(0xD6D0), Some(0x4E2D)); // 中
        assert_eq!(lookup(0xC7EF), Some(0x79CB));
        assert_eq!(lookup(0xC1AC), Some(0x8FDE));
        assert_eq!(lookup(0xB2CB), Some(0x83DC));
        assert_eq!(lookup(0xB4F3), Some(0x5927));
        assert_eq!(lookup(0xA1A1), Some(0x3000));
        // 表外码位（0x7F 尾字节区被生成器排除）与 ASCII 区均查不到。
        assert_eq!(lookup(0x817F), None);
        assert_eq!(lookup(0x0041), None);
    }

    #[test]
    fn frozen_table_matches_the_jdk_dump_pair_for_pair() {
        // 全表对拍（M2-12 collation 先例的纪律：机械证据，不是抽查）——
        // gbk.bin 必须与 ProbeEncoding.java 的 dump **逐对全等**：
        // 生成器只加头不改内容，这个测试钉死"表 == JDK 真值"。
        let dump: &[u8] = include_bytes!("../../../tests/fixtures/gbk_jdk.bin");
        assert_eq!(dump.len() % 4, 0, "dump 应是 (u16,u16) 对序列");
        let pairs = dump.len() / 4;
        let body = &GBK_TABLE[HEADER_LEN..];
        assert_eq!(
            body.len(),
            pairs * 4,
            "gbk.bin 条目数应与 dump 一致（表被意外改过？）"
        );
        for i in 0..pairs {
            let off = i * 4;
            assert_eq!(
                &body[off..off + 4],
                &dump[off..off + 4],
                "第 {i} 对与 JDK dump 不一致"
            );
        }
        // 头部自校验（生成器格式契约：magic + count）。
        assert_eq!(&GBK_TABLE[..8], b"RPXGBK\x01\x00");
        let count = u32::from_be_bytes([GBK_TABLE[8], GBK_TABLE[9], GBK_TABLE[10], GBK_TABLE[11]]);
        assert_eq!(count as usize, pairs);
        // 规模自校验：探针日志 GBK_DUMP_PAIRS 的记录值。
        assert_eq!(pairs, 23940);
    }

    #[test]
    fn ascii_bytes_pass_through_and_double_bytes_decode() {
        // 混合 ASCII 与 GBK 双字节：North-华北-100。
        let mut bytes = b"North-".to_vec();
        bytes.extend_from_slice(&[0xD6, 0xD0]); // 中
        bytes.extend_from_slice(b"-");
        bytes.extend_from_slice(&[0xB1, 0xB1]); // 北
        bytes.extend_from_slice(b"-100\r\n");
        assert_eq!(decode_gbk(&bytes).unwrap(), "North-中-北-100\r\n");
    }

    #[test]
    fn malformed_sequences_fail_explicitly() {
        // 孤立高字节（0x80 不在 lead 区）。
        assert!(decode_gbk(&[0x80, b'A']).is_err());
        // 尾字节越界（0x20 < 0x40）。
        assert!(decode_gbk(&[0xD6, 0x20]).is_err());
        // 表外序列：lead/trail 合法但组合未定义（0x81 0x30 是 GB18030 四字节区前缀）。
        assert!(decode_gbk(&[0x81, 0x30]).is_err());
        // 截断：文件以前导字节结尾。
        assert!(decode_gbk(&[0xD6]).is_err());
    }

    #[test]
    fn encoding_name_dispatch_matches_charset_for_name_semantics() {
        // 缺省 = 产品默认 GBK。
        let gbk_bytes = [0xD6u8, 0xD0];
        assert_eq!(decode(&gbk_bytes, None).unwrap(), "中");
        assert_eq!(decode(&gbk_bytes, Some("")).unwrap(), "中");
        // 大小写不敏感（Charset.forName 语义）。
        for name in ["GBK", "gbk", "Gbk", "CP936", "windows-936"] {
            assert_eq!(decode(&gbk_bytes, Some(name)).unwrap(), "中", "{name}");
        }
        // UTF-8 显式声明走 from_utf8（兼容既有夹具路径）。
        assert_eq!(decode("中".as_bytes(), Some("UTF-8")).unwrap(), "中");
        assert!(decode(&[0xFF, 0xFE], Some("UTF-8")).is_err());
        // 不支持的 charset 名显式失败（GB18030 四字节场景在此带账）。
        assert!(matches!(
            decode(b"x", Some("Big5")),
            Err(EngineError::UnsupportedEncoding { .. })
        ));
    }
}
