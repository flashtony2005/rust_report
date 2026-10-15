//! 单元格样式（定义 JSON → 模板格）：字体 / 字号 / 粗斜下划线 / 前景背景色。
//!
//! 与 [`crate::snapshot::style`]（模板格 → Univer `IStyleData`）互为**逆**：那边读
//! `Cell` 出 `ff`/`fs`/`bl`/`it`/`ul`/`cl`/`bg`，这边把 JSON 写进 `Cell`。因此端到端
//! 判据天然闭环 —— **JSON 写进去的样式必须原样出现在 `/render` 快照里**（映射表见
//! `docs/spec-m4.md` §4.3，字段表见 `docs/report-def-json.md` §3.2）。
//!
//! 样式**只改 7 个槽**，其余一律走 [`crate::reportdef::blank_cell`] 的契约默认
//! （附件 B.3）。边框 / 数字格式 / 缩进 / 旋转仍是 v1 范围之外（契约 §7.2）。

use serde_json::{Map, Value as J};

use rpx_core::cell::Cell;
use rpx_core::props::bit;

use super::json::{as_str, as_usize, err, kind_of, opt_str};
use super::DefError;

/// 把 `cells[i]` 上的样式字段写进模板格（未声明的字段保持默认）。
///
/// 逐字段独立：给了才改，不给不动 —— 因此「只声明 `bold`」不会把字号/颜色打回默认。
pub(super) fn apply_style(cell: &mut Cell, o: &Map<String, J>, path: &str) -> Result<(), DefError> {
    if let Some(f) = opt_str(o, "font_name", path)? {
        cell.font_name = Some(f.to_string());
    }
    if let Some(v) = o.get("font_size") {
        let p = format!("{path}.font_size");
        let n = as_usize(v, &p)?;
        if n == 0 || n > i16::MAX as usize {
            return Err(err(
                &p,
                format!("字号 {n} 越界（1..={}，即格式的 i16 上限）", i16::MAX),
            ));
        }
        cell.font_size = n as i16;
    }
    // 三个位标志（附件 B.3.1）：`true` 置位、`false` 清位（默认即清）。
    for (key, mask) in [
        ("bold", bit::BOLD),
        ("italic", bit::ITALIC),
        ("underline", bit::UNDERLINE),
    ] {
        if let Some(v) = o.get(key) {
            let p = format!("{path}.{key}");
            let on = v
                .as_bool()
                .ok_or_else(|| err(&p, format!("应为布尔，实得 {}", kind_of(v))))?;
            set_flag(cell, mask, on);
        }
    }
    if let Some(v) = o.get("fore_color") {
        let p = format!("{path}.fore_color");
        cell.fore_color = parse_color(as_str(v, &p)?, &p)?;
    }
    if let Some(v) = o.get("back_color") {
        let p = format!("{path}.back_color");
        cell.back_color = parse_color(as_str(v, &p)?, &p)?;
    }
    Ok(())
}

/// 位标志置/清（`props::bit` 的位打包，附件 B.3.1）。
fn set_flag(cell: &mut Cell, mask: u16, on: bool) {
    if on {
        cell.bit_flags |= mask;
    } else {
        cell.bit_flags &= !mask;
    }
}

/// `"#RRGGBB"` → ARGB int（alpha 固定 `FF`）。
///
/// 只认 **6 位十六进制**（大小写均可）—— 这正是 `/render` 快照回吐的形态
/// （[`crate::snapshot::style::argb_hex`] 取低 24 位），两边严格互逆。
/// 带 alpha 的 8 位形态是 v1 范围之外（契约 §7.2），未知形态**显式报错不猜**。
///
/// `pub(super)`：边框的 `color` 槽（`cellprops.rs`）共用同一个解析器 ——
/// 颜色编码只有一处实现，互逆保证也只有一处需要守。
pub(super) fn parse_color(s: &str, path: &str) -> Result<i32, DefError> {
    let hex = s.strip_prefix('#').ok_or_else(|| {
        err(
            path,
            format!("颜色应为 \"#RRGGBB\"，实得 {s:?}（缺 `#` 前缀）"),
        )
    })?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(err(
            path,
            format!("颜色应为 \"#RRGGBB\"（6 位十六进制），实得 {s:?}"),
        ));
    }
    // 已校验：恰 6 位十六进制 ⇒ `from_str_radix` 不可能失败。
    let rgb = u32::from_str_radix(hex, 16).expect("6 位十六进制已校验");
    Ok((0xFF00_0000u32 | rgb) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::style::argb_hex;
    use rpx_core::props::def;

    fn apply(json: &str) -> Cell {
        let o: Map<String, J> = serde_json::from_str(json).expect("测试 JSON 合法");
        let mut c = crate::reportdef::blank_cell();
        apply_style(&mut c, &o, "$.cells[0]").expect("样式应被接受");
        c
    }

    /// 颜色与 `/render` 快照的 `argb_hex` **严格互逆**（判据闭环的关键）。
    #[test]
    fn color_round_trips_through_the_snapshot_hex() {
        for (src, argb) in [
            ("#000000", def::COLOR_BLACK),
            ("#FFFFFF", def::COLOR_WHITE),
            ("#123456", 0xFF12_3456u32 as i32),
        ] {
            let parsed = parse_color(src, "$.p").expect("合法颜色");
            assert_eq!(parsed, argb, "{src}");
            assert_eq!(argb_hex(parsed), src, "回吐必须原样（大写十六进制）");
        }
    }

    #[test]
    fn color_accepts_lowercase_and_rejects_malformed() {
        assert_eq!(
            parse_color("#abcdef", "$.p").unwrap(),
            parse_color("#ABCDEF", "$.p").unwrap()
        );
        for bad in ["123456", "#12345", "#1234567", "#GGGGGG", "#"] {
            assert!(parse_color(bad, "$.p").is_err(), "{bad} 应被拒");
        }
    }

    /// 逐字段独立：只声明 `bold` 不得动字号 / 颜色（未声明即保持默认）。
    #[test]
    fn only_declared_fields_change() {
        let c = apply(r#"{"bold": true}"#);
        assert!(c.bold());
        assert_eq!(c.font_size, def::FONT_SIZE);
        assert_eq!(c.fore_color, def::COLOR_BLACK);
        assert_eq!(c.back_color, def::COLOR_WHITE);
        assert!(!c.italic() && !c.underline());
    }

    #[test]
    fn flags_set_and_clear_independently() {
        let c = apply(r#"{"bold": true, "italic": true, "underline": true}"#);
        assert!(c.bold() && c.italic() && c.underline());
        assert!(c.visible(), "visible 位（默认置位）不得被样式误清");
        let c = apply(r#"{"bold": false, "italic": false}"#);
        assert!(!c.bold() && !c.italic() && !c.underline());
    }

    #[test]
    fn font_size_bounds_are_the_format_i16_range() {
        assert_eq!(apply(r#"{"font_size": 1}"#).font_size, 1);
        assert_eq!(apply(r#"{"font_size": 32767}"#).font_size, 32767);
        let o: Map<String, J> = serde_json::from_str(r#"{"font_size": 0}"#).unwrap();
        let mut c = crate::reportdef::blank_cell();
        assert!(apply_style(&mut c, &o, "$.c").is_err(), "0 号字应被拒");
    }

    /// 错误的 JSON 路径必须指向真实字段（前端据此定位）。
    #[test]
    fn errors_carry_the_field_path() {
        let o: Map<String, J> = serde_json::from_str(r#"{"fore_color": "red"}"#).unwrap();
        let mut c = crate::reportdef::blank_cell();
        let e = apply_style(&mut c, &o, "$.cells[7]").unwrap_err();
        assert_eq!(
            format!("{e}"),
            "$.cells[7].fore_color：颜色应为 \"#RRGGBB\"，实得 \"red\"（缺 `#` 前缀）"
        );
    }
}
