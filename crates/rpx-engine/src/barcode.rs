//! Code 128（子集 B）→ 1 位灰度 PNG data URI。
//!
//! # 为什么自己写编码器和 PNG
//!
//! * 引擎是**零依赖层**（`docs/code-review.md` §3.2：`rpx-core` + `rpx-engine` 只许 path
//!   依赖），不能引 `png` / `image` / `qrcode` 之类的 crate。
//! * `rpx-export::validate_image` 只收 **PNG / JPEG / GIF / BMP**（xlsx 嵌入要真位图），
//!   引擎侧 `graphics::calculate::image` 也做同样的前缀白名单 ⇒ **SVG data URI 会被
//!   两处拒掉**，所以这里必须产出 PNG，不能图省事输出 SVG。
//!
//! # PNG 用 stored（未压缩）deflate
//!
//! zlib 允许 `BTYPE=00` 的块直接存原始字节，于是**不需要任何 deflate 实现**，
//! 只需要 CRC-32 与 Adler-32 两个校验。代价是体积：一条 24 字符的 Code128 是
//! 319×40 像素，1 位灰度原图 ≈ 1.6 KB，base64 后 ≈ 2.1 KB。标签条码完全可接受。
//!
//! # 图案表来源
//!
//! 107 行逐条对齐 Code 128 规范的 Widths 列（Wikipedia `Code_128` 表；
//! 2026-10-18 抓取核对）。表里每行是 6 个元素宽度（条/空交替，从条开始），
//! 除 Stop（106）是 7 个元素。**每个非 Stop 行的数字和必须等于 11**，
//! Stop 等于 13 —— 见 [`tests::pattern_table_is_well_formed`]。
//!
//! ⛔ 这张表错一位的症状是「能渲染、能看出是条码、但扫不出来」，属于最难发现的
//! 静默错误。改表必须同时更新校验和测试与 [`tests::known_code_b_sequence`]。

/// 静区（左右各 10 模块，Code 128 规范要求 ≥10）。
const QUIET: usize = 10;

const START_B: u8 = 104;
const STOP: u8 = 106;

/// Code 128 图案表（值 0..=106）。
#[rustfmt::skip]
const PATTERNS: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312",
    "132212", "221213", "221312", "231212", "112232", "122132", "122231", "113222",
    "123122", "123221", "223211", "221132", "221231", "213212", "223112", "312131",
    "311222", "321122", "321221", "312212", "322112", "322211", "212123", "212321",
    "232121", "111323", "131123", "131321", "112313", "132113", "132311", "211313",
    "231113", "231311", "112133", "112331", "132131", "113123", "113321", "133121",
    "313121", "211331", "231131", "213113", "213311", "213131", "311123", "311321",
    "331121", "312113", "312311", "332111", "314111", "221411", "431111", "111224",
    "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114",
    "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111",
    "111242", "121142", "121241", "114212", "124112", "124211", "411212", "421112",
    "421211", "212141", "214121", "412121", "111143", "111341", "131141", "114113",
    "114311", "411113", "411311", "113141", "114131", "311141", "411131", "211412",
    "211214", "211232", "2331112",
];

/// 把文本编成 Code 128B 符号序列（含 Start / 校验 / Stop）。
///
/// 校验位 = `(104 + Σ 值_i × i) % 103`，`i` 从 1 起（Start 不参与加权）。
fn symbols(text: &str) -> Result<Vec<u8>, String> {
    let mut out = vec![START_B];
    let mut sum = u32::from(START_B);
    for (i, ch) in text.chars().enumerate() {
        let code = ch as u32;
        if !(32..=126).contains(&code) {
            return Err(format!(
                "Code 128B 只能编码 ASCII 32..126，遇到 {ch:?}（U+{code:04X}）"
            ));
        }
        let value = (code - 32) as u8;
        out.push(value);
        sum += u32::from(value) * (i as u32 + 1);
    }
    out.push((sum % 103) as u8);
    out.push(STOP);
    Ok(out)
}

/// 符号序列 → 模块位图（`true` = 空白，`false` = 条）。
fn modules(syms: &[u8]) -> Vec<bool> {
    let mut out = vec![true; QUIET];
    for &symbol in syms {
        let mut bar = true;
        for byte in PATTERNS[symbol as usize].bytes() {
            let width = usize::from(byte - b'0');
            out.extend(std::iter::repeat_n(!bar, width));
            bar = !bar;
        }
    }
    out.extend(std::iter::repeat_n(true, QUIET));
    out
}

/// CRC-32（PNG 分块用，多项式 0xEDB88320）。
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Adler-32（zlib 尾校验）。
fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// zlib 流：2 字节头 + 若干 stored 块 + Adler-32。
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    if raw.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    }
    for (i, block) in raw.chunks(65_535).enumerate() {
        let last = (i + 1) * 65_535 >= raw.len();
        out.push(u8::from(last));
        let len = block.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = Vec::with_capacity(4 + data.len());
    body.extend_from_slice(kind);
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// 1 位灰度 PNG。`bits` 是行优先的「是否白」位图。
fn png_1bit(width: u32, height: u32, bits: &[bool]) -> Vec<u8> {
    let stride = width.div_ceil(8) as usize;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0); // 过滤器：none
        let mut row = vec![0xFFu8; stride]; // 未用位补白，避免右边缘出现黑边
        for x in 0..width as usize {
            if !bits[y * width as usize + x] {
                row[x / 8] &= !(0x80 >> (x % 8));
            }
        }
        raw.extend_from_slice(&row);
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[1, 0, 0, 0, 0]); // 位深 1 / 灰度 / 压缩 0 / 过滤 0 / 隔行 0
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// 标准 base64（带 `=` 填充）。
fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for group in data.chunks(3) {
        let b = [
            group[0],
            *group.get(1).unwrap_or(&0),
            *group.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= group.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * i)) as usize & 0x3F]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// 文本 → Code 128B 条码 PNG data URI。
///
/// `module` 是单个模块的像素宽度（1 起），`height` 是像素高度。
pub fn data_uri(text: &str, module: u32, height: u32) -> Result<String, String> {
    if text.is_empty() {
        return Err("条码内容为空".into());
    }
    if !(1..=16).contains(&module) {
        return Err("模块宽度应在 1..=16 像素内".into());
    }
    if !(1..=2048).contains(&height) {
        return Err("条码高度应在 1..=2048 像素内".into());
    }
    let syms = symbols(text)?;
    let bar = modules(&syms);
    let width = (bar.len() as u32) * module;
    let mut bits = Vec::with_capacity((width * height) as usize);
    for _ in 0..height {
        for white in &bar {
            bits.extend(std::iter::repeat_n(*white, module as usize));
        }
    }
    Ok(format!(
        "data:image/png;base64,{}",
        base64(&png_1bit(width, height, &bits))
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 图案表自检：非 Stop 每行数字和 = 11 模块，Stop = 13。
    ///
    /// 这条能抓住「少写/多写一位」和大部分「数字改错」，抓不住同和置换 ——
    /// 后者靠 [`known_code_b_sequence`] 与真实扫码验。
    #[test]
    fn pattern_table_is_well_formed() {
        for (i, pattern) in PATTERNS.iter().enumerate() {
            let sum: u32 = pattern.bytes().map(|b| u32::from(b - b'0')).sum();
            let (want_sum, want_len) = if i == STOP as usize { (13, 7) } else { (11, 6) };
            assert_eq!(sum, want_sum, "值 {i} 的模块和应为 {want_sum}：{pattern}");
            assert_eq!(pattern.len(), want_len, "值 {i} 的元素个数应为 {want_len}");
        }
    }

    /// 已知序列：`"A"` → Start B(104) / A(33) / 校验 34 / Stop(106)。
    ///
    /// 校验 = `(104 + 33×1) % 103 = 137 % 103 = 34`。
    #[test]
    fn known_code_b_sequence() {
        assert_eq!(symbols("A").unwrap(), vec![104, 33, 34, 106]);
        // 多字符：`(104 + 33×1 + 34×2) % 103 = 205 % 103 = 102`。
        assert_eq!(symbols("AB").unwrap(), vec![104, 33, 34, 102, 106]);
    }

    #[test]
    fn non_ascii_is_rejected() {
        assert!(symbols("华东").is_err());
        assert!(symbols("A\n").is_err());
    }

    /// 产出必须是结构合法的 PNG：签名 / IHDR 尺寸 / IEND 齐全。
    #[test]
    fn output_is_a_png_data_uri() {
        let uri = data_uri("N000000021000111-JIJIA-1", 1, 40).unwrap();
        let encoded = uri
            .strip_prefix("data:image/png;base64,")
            .expect("必须是 PNG data URI");
        let png = decode(encoded);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        // 24 字符 ⇒ Start + 24 + 校验 = 26 个 11 模块符号 + Stop 13 模块 = 299，
        // 加左右静区各 10 ⇒ 319。
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 319);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 40);
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
    }

    /// 测试用的最小 base64 解码（只认标准表 + `=`，与 `base64()` 对称）。
    fn decode(text: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let mut acc = 0u32;
        let mut bits = 0u32;
        for ch in text.bytes() {
            let v = match ch {
                b'A'..=b'Z' => ch - b'A',
                b'a'..=b'z' => ch - b'a' + 26,
                b'0'..=b'9' => ch - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => break,
                other => panic!("非法 base64 字符 {other}"),
            };
            acc = (acc << 6) | u32::from(v);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
            }
        }
        out
    }
}
