//! 夹具 `expected.json` 的极小子集扫描器（零第三方依赖 —— 本 crate 的硬门禁）。
//!
//! 夹具 JSON 由 `Json.java` 生成：**键序固定、无空白**，所以按标记切片即可。
//! 两处必须先想清楚的坑（本轮实测各踩过一次）：
//!
//! 1. ⛔ **不能按 `},{` 切格**：`sub_values` 内部也是 `},{`，会把锚点格劈成两半，
//!    于是「克隆行集合」永远为空 → 未移动区判据整体失效（假通过）。按**括号深度**切。
//! 2. `"cells"` 这个名字出现两次（`calculated` 与 `merged`），要用 `"pagination"`
//!    之后的 `merged` 段来界定终点。

use std::collections::BTreeSet;

use crate::value::Value;

/// 夹具里的一格（只取对拍用到的字段）。
#[derive(Debug)]
pub(super) struct ExpCell {
    pub(super) r: i32,
    pub(super) c: i32,
    /// `value` 的**原始 JSON token**（原样文本：字符串含引号、数字无引号）。
    /// ⛔ 必须按原文比：`100.00` 经 JSON 解析会变成 f64 `100.0`，精度信息就丢了
    /// （`engine_types` 的 decimal 钉子正是靠这个才成立）。
    pub(super) value: Option<String>,
    /// `type` 字段（未加引号的短名，如 `int` / `decimal` / `string`）。
    pub(super) ty: Option<String>,
    /// `disp_value`（`String.valueOf(value)` 不同时才落盘，`CalcJson.java:71-73`）。
    pub(super) disp: Option<String>,
    /// `source` → (src_row, src_col, row_merge, col_merge, extend)
    pub(super) src: Option<(i32, i32, i32, i32, i32)>,
    /// `left_head` → (src_row, src_col)
    pub(super) lh: Option<(i32, i32)>,
    /// `sub_values` 的 (r, c) 列表（`subs > 1` 才有）
    pub(super) subs: Vec<(i32, i32)>,
}

pub(super) fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

pub(super) fn expected_text(name: &str) -> String {
    let path = fixtures_dir().join(format!("{name}.expected.json"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("读取 {name}.expected.json 失败：{e}"))
}

/// 解析 `calculated.cells` 数组。
pub(super) fn parse_expected(text: &str) -> Vec<ExpCell> {
    let marker = "\"cells\":[";
    let body_start = text.find(marker).expect("夹具缺 \"cells\":[") + marker.len();
    let tail = text.find("\"pagination\"").expect("夹具缺 pagination 段");
    let body_end = text[..tail].rfind("]}").expect("夹具缺 cells 段收尾");
    let body = &text[body_start..body_end];
    if body.is_empty() {
        return Vec::new(); // "cells":[]
    }
    split_top_level(body).into_iter().map(parse_cell).collect()
}

/// 按 `{...}` 深度把 JSON 数组体切成顶层元素（忽略字符串内的括号与转义）。
fn split_top_level(body: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    let (mut in_str, mut escaped) = (false, false);
    for (i, ch) in body.char_indices() {
        if in_str {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    out.push(&body[start..=i]);
                    start = i + 2; // 跳过元素间的 `,`
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_cell(chunk: &str) -> ExpCell {
    let r = int_after(chunk, "\"r\":").expect("格缺 r") as i32;
    let c = int_after(chunk, "\"c\":").expect("格缺 c") as i32;
    let value = raw_token_after(chunk, "\"value\":");
    let ty = raw_token_after(chunk, "\"type\":").map(unquote);
    let disp = raw_token_after(chunk, "\"disp_value\":").map(unquote);
    let src = slice_from(chunk, "\"source\":{").map(|s| {
        (
            int_after(s, "\"src_row\":").expect("src_row") as i32,
            int_after(s, "\"src_col\":").expect("src_col") as i32,
            int_after(s, "\"row_merge\":").expect("row_merge") as i32,
            int_after(s, "\"col_merge\":").expect("col_merge") as i32,
            int_after(s, "\"extend\":").expect("extend") as i32,
        )
    });
    let lh = slice_from(chunk, "\"left_head\":{").map(|s| {
        (
            int_after(s, "\"src_row\":").expect("lh.src_row") as i32,
            int_after(s, "\"src_col\":").expect("lh.src_col") as i32,
        )
    });
    let subs = match slice_from(chunk, "\"sub_values\":[") {
        None => Vec::new(),
        Some(s) => {
            let stop = s.find("]}").unwrap_or(s.len());
            s[..stop]
                .split("},{")
                .map(|x| {
                    (
                        int_after(x, "\"r\":").expect("sub.r") as i32,
                        int_after(x, "\"c\":").expect("sub.c") as i32,
                    )
                })
                .collect()
        }
    };
    ExpCell {
        r,
        c,
        value,
        ty,
        disp,
        src,
        lh,
        subs,
    }
}

/// `"key":` 之后的**原样 token**（字符串**保留**引号、并解掉转义；数字/布尔/null 原样）。
///
/// 保留引号是刻意的：`value` 段必须按原文比 —— 夹具里 `"1000"`（字符串）与 `1000`
/// （数字）是两种 token，脱掉引号就没法区分"字符串 vs 数字"的判据了。
///
/// ⛔ 非 ASCII **必须解转义**：夹具用 `Json.quote` 生成，中文会写成 `\u5317\u533a`
/// （`engine_disp` 的 `北区` 就是），按字面读会得到 `u5317u533a`。
fn raw_token_after(text: &str, key: &str) -> Option<String> {
    let at = text.find(key)? + key.len();
    let rest = &text[at..];
    let Some(body) = rest.strip_prefix('"') else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        return Some(rest[..end].to_string());
    };
    // 先切出「未转义的收尾引号」之前的原始体（反斜杠按奇偶配对，`\"` 不算收尾）
    let (mut escaped, mut end) = (false, None);
    for (i, ch) in body.char_indices() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            end = Some(i);
            break;
        }
    }
    let raw = &body[..end?];
    Some(format!("\"{}\"", unescape_json(raw)?))
}

/// 解 JSON 转义（含 `\uXXXX` 与代理对）—— 夹具值由 Java `Json.quote` 生成，
/// 非 BMP 字符会写成两个 `\u` 转义。
fn unescape_json(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut it = raw.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match it.next()? {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'u' => {
                let code =
                    u32::from_str_radix(&it.by_ref().take(4).collect::<String>(), 16).ok()?;
                if (0xD800..0xDC00).contains(&code) {
                    // 高代理：后随 `\uDC00..\uDFFF` 才拼得回一个字符
                    let mut probe = it.clone();
                    if probe.next() == Some('\\') && probe.next() == Some('u') {
                        let hex: String = probe.by_ref().take(4).collect();
                        if let Ok(lo) = u32::from_str_radix(&hex, 16) {
                            if (0xDC00..0xE000).contains(&lo) {
                                it = probe;
                                let c = 0x10000 + ((code - 0xD800) << 10) + (lo - 0xDC00);
                                out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
                                continue;
                            }
                        }
                    }
                    out.push('\u{FFFD}');
                } else {
                    out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                }
            }
            other => out.push(other),
        }
    }
    Some(out)
}

/// 去掉 `raw_token_after` 结果的外层引号（用于 `type` / `disp_value` 这类纯文本字段）。
fn unquote(token: String) -> String {
    token
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .map(|s| s.to_string())
        .unwrap_or(token)
}

/// 从标记处起的尾串（标记缺失 → `None`）。
fn slice_from<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let at = text.find(marker)? + marker.len();
    Some(&text[at..])
}

/// 取 `"key":` 之后的第一个整数（键序固定，故首个出现即所求）。
fn int_after(text: &str, key: &str) -> Option<i64> {
    let at = text.find(key)? + key.len();
    let rest = &text[at..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// 夹具声明的模板尺寸（逻辑口径，不含 0 号脚手架）。
pub(super) fn template_dims(text: &str) -> (i32, i32) {
    (
        int_after(text, "\"template_rows\":").expect("template_rows") as i32,
        int_after(text, "\"template_cols\":").expect("template_cols") as i32,
    )
}

/// 从 `sub_values` 推出**被克隆出来的行/列**（`sub.r != cell.r` 即克隆行）。
///
/// 这是"哪些坐标已经不是模板坐标"的判据：插入行/列之后的区域不能用阶段 B
/// 的模板形状产物去比。给不出证据的夹具返回空集合（判据退化为全网格比对）。
pub(super) fn clone_axes(cells: &[ExpCell]) -> (BTreeSet<i32>, BTreeSet<i32>) {
    let mut rows = BTreeSet::new();
    let mut cols = BTreeSet::new();
    for e in cells {
        for (sr, sc) in &e.subs {
            if *sr != e.r {
                rows.insert(*sr);
            }
            if *sc != e.c {
                cols.insert(*sc);
            }
        }
    }
    (rows, cols)
}

/// [`Value`] → 夹具 `value` 段的**原样 token**（复刻 `CalcJson.value():219-254`）。
///
/// ⛔ 这是**写入侧的展示约定**，不是引擎语义：整数型浮点会被塌成整数字面量
/// （`Double(100.0)` → `100`），所以**只靠 JSON 原文反推类型是错的** ——
/// 类型必须比 `type` 字段（`engine_types` 的 `(2,4)` 就是这条钉子）。
pub(super) fn fixture_value_token(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Text(s) => format!("\"{s}\""),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Long(l) => l.to_string(),
        Value::Short(s) => s.to_string(),
        Value::Byte(b) => b.to_string(),
        Value::Decimal(d) => d.clone(),
        Value::Double(d) => number_token(*d),
        Value::Float(f) => number_token(f64::from(*f)),
        Value::List(_) => panic!("List 的值列在 sub_values，本渲染器不覆盖"),
    }
}

/// `CalcJson.value()` 的浮点分支：整数型且 `|d| < 1e15` → 整数形态，否则 `toString`。
fn number_token(d: f64) -> String {
    if d == d.floor() && d.abs() < 1e15 {
        (d as i64).to_string()
    } else {
        format!("{d}")
    }
}

#[cfg(test)]
mod tests;
