//! 单元格属性扩展（定义 JSON → 模板格）：**边框 / 数字格式 / 缩进 / 旋转 / 扩展模式 /
//! 声明式主格**。
//!
//! 与 [`super::style`]（字体字号颜色粗斜下划线）互补 —— 那边 7 个槽，这边 18 个：
//! 边框 12（4 边 × style/width/color）+ `format` + `indent` + `rotation_angle` +
//! `extend_mode` + `left_head` + `top_head`。
//!
//! # 声明式主格（`left_head` / `top_head`）为什么必须在这里落地
//!
//! 引擎**早已**支持它：`stage_b.rs:282-283` 把 `Cell.left_head` / `Cell.top_head`
//! 原样搬进 `HeadCell.declared_left` / `declared_top`，`heads/find.rs` 的**分支 A**
//! 优先读它们（`ExtCellSet.java:633-647` 的逐行移植）。缺的只是这一层
//! —— 在 M7 之前，JSON 装载器**没有这两个字段**，于是每一格的 `left_head`/`top_head`
//! 恒为 `None`，分支 A 永远走不到。
//!
//! ⚠ **分支 A 的失败是静默的**：`find.rs:96` 是 `if !head.is_none()` —— 声明的地址
//! 若解析不到格，**不报错**，直接落到分支 B 的自动推导。所以「打错一个格号」
//! 与「不声明」在引擎侧完全同形。本模块因此**在装载期就把地址形状钉死**
//! （`heads::parse_declared_head`），并且由 [`super::parse`] 在全部格落盘后**再查一次存在性**
//! —— 让打错格号变成一条带 JSON 路径的显式错误，而不是一个悄悄算错的数。
//!
//! 教程依据（`008_jcbb14`「组表头重复」整条链：`A3.左主格=B3`、`B3.左主格=A2`、
//! `C4.左主格=B3` …）与设计文档「主格和附属格认定规则」的**人为认定**分支。
//!
//! # 为什么验证走**模型往返**而不是快照读回
//!
//! `snapshot::style::style_json` 只暴露 9 个字段（`bl/it/ul/ff/fs/cl/bg/ht/vt`），
//! 边框/格式/缩进/旋转**不在其中**（`spec-m4.md` §4.3 明确列为后置）。所以端到端
//! 读回走不通，判据改为 **`open_bytes(to_bytes(build(json))) == build(json)`**：
//! 经真实 `.rpx` 字节闭合「解析 → 写侧 → 读侧」整圈。这是 M7-2 已立下的判据形态
//! （`rpx-core/tests/roundtrip.rs`），此处沿用到 JSON 侧。
//!
//! 契约正文见 `docs/report-def-json.md` §3.2 / §4.7。

use serde_json::{Map, Value as J};

use rpx_core::cell::Cell;
use rpx_core::props::def;

use super::json::{as_f32, as_str, err, field, obj, opt_f32};
use super::style::parse_color;
use super::DefError;

/// 边框线型枚举名 → 码。
///
/// 出处 `docs/format-contract.md:419`（`NormalCell.java:1316-1336`）：
/// `0`=none、`81`='Q'=dotted、`82`='R'=dashed、`83`='S'=solid、`84`='T'=double、
/// `85`='U'=dotted；未知值按 solid。
///
/// **`85` 不开放**：契约自己写明它与 `81` 同为 dotted（Java 侧的重复码），
/// 开放两个名字对应同一视觉只会让人猜「到底哪个对」。读侧 `props::normalize_style`
/// 还把 `80`（'P'）归一成 `0`，那是 Java 的写入怪癖，同样不开放。
const BORDER_STYLES: &[(&str, u8)] = &[
    ("none", 0),
    ("dotted", 81),
    ("dashed", 82),
    ("solid", 83),
    ("double", 84),
];

/// `extend_mode` 枚举名 → 码。
///
/// 出处 `docs/format-contract.md` 注 3（`NormalCell.java:656`）：`16`=默认/自动、
/// `17`=不扩展、`18`=扩展（纵向）、`19`=扩展（横向判定）。
/// **18/19 的方向已由夹具实证**：`engine_ext`（19 → 横向铺开，列数 ×N）与
/// `engine_head_left`（18 → 纵向下推），见契约 §… 的扩展矩阵。
const EXTEND_MODES: &[(&str, i8)] = &[
    ("default", def::EXTEND_DEFAULT),
    ("none", def::EXTEND_NONE),
    ("vertical", def::EXTEND_VERTICAL),
    ("horizontal", def::EXTEND_HORIZONTAL),
];

/// 把 `cells[i]` 上的属性扩展字段写进模板格（未声明的保持 [`super::blank_cell`] 默认）。
///
/// 逐字段独立，语义同 [`super::style::apply_style`]。
pub(super) fn apply_cell_props(
    cell: &mut Cell,
    o: &Map<String, J>,
    path: &str,
) -> Result<(), DefError> {
    for (name, flag) in [
        (
            "stretch_when_paged",
            rpx_core::props::bit::STRETCH_WHEN_PAGED,
        ),
        ("split", rpx_core::props::bit::SPLITTED),
    ] {
        if let Some(value) = o.get(name) {
            let enabled = value
                .as_bool()
                .ok_or_else(|| err(&format!("{path}.{name}"), "需要布尔值"))?;
            if enabled {
                cell.bit_flags |= flag
            } else {
                cell.bit_flags &= !flag
            }
        }
    }
    if let Some(b) = o.get("border") {
        apply_border(cell, b, path)?;
    }
    if let Some(f) = o.get("format") {
        let p = format!("{path}.format");
        cell.format = Some(as_str(f, &p)?.to_string());
    }
    if let Some(i) = o.get("indent") {
        let p = format!("{path}.indent");
        let n = as_f32(i, &p)?;
        if n < 0.0 {
            return Err(err(&p, format!("缩进不得为负（实得 {n}）")));
        }
        cell.indent = n;
    }
    if let Some(r) = o.get("rotation") {
        let p = format!("{path}.rotation");
        let n = as_f32(r, &p)?;
        if n < 0.0 {
            return Err(err(&p, format!("旋转角不得为负（实得 {n}）")));
        }
        cell.rotation_angle = n;
    }
    if let Some(e) = o.get("extend_mode") {
        let p = format!("{path}.extend_mode");
        cell.extend_mode = lookup_i8(EXTEND_MODES, as_str(e, &p)?, &p)?;
    }
    // 声明式主格：两槽独立（一格可**同时**有左主格与上主格 —— 交叉表正是靠这个）。
    if let Some(h) = o.get("left_head") {
        let p = format!("{path}.left_head");
        cell.left_head = Some(super::heads::parse_declared_head(as_str(h, &p)?, &p)?);
    }
    if let Some(h) = o.get("top_head") {
        let p = format!("{path}.top_head");
        cell.top_head = Some(super::heads::parse_declared_head(as_str(h, &p)?, &p)?);
    }
    if let Some(v) = o.get("hyperlink") {
        let p = format!("{path}.hyperlink");
        let url = as_str(v, &p)?;
        if url.is_empty() {
            return Err(err(&p, "超链接不能为空"));
        }
        cell.hyperlink = Some(url.to_string());
    }
    if let Some(v) = o.get("hyperlink_window") {
        let p = format!("{path}.hyperlink_window");
        cell.hyperlink_window = Some(as_str(v, &p)?.to_string());
    }
    if let Some(v) = o.get("notes") {
        let p = format!("{path}.notes");
        cell.notes = Some(as_str(v, &p)?.to_string());
    }
    Ok(())
}

/// `border`：四条边各自可选，边内 `style` 必填、`width`/`color` 可选。
///
/// 四条边落到**三组不同名**的槽（`lb_*` / `rb_*` / `tb_*` / `bb_*`），
/// 所以不能像位标志那样用 mask 循环 —— 逐边显式赋值（宁可重复，不可错位）。
fn apply_border(cell: &mut Cell, v: &J, path: &str) -> Result<(), DefError> {
    let bp = format!("{path}.border");
    let b = obj(v, &bp)?;

    if let Some(s) = b.get("left") {
        let (st, w, c) = parse_side(s, &format!("{bp}.left"))?;
        cell.lb_style = st;
        cell.lb_width = w;
        cell.lb_color = c;
    }
    if let Some(s) = b.get("right") {
        let (st, w, c) = parse_side(s, &format!("{bp}.right"))?;
        cell.rb_style = st;
        cell.rb_width = w;
        cell.rb_color = c;
    }
    if let Some(s) = b.get("top") {
        let (st, w, c) = parse_side(s, &format!("{bp}.top"))?;
        cell.tb_style = st;
        cell.tb_width = w;
        cell.tb_color = c;
    }
    if let Some(s) = b.get("bottom") {
        let (st, w, c) = parse_side(s, &format!("{bp}.bottom"))?;
        cell.bb_style = st;
        cell.bb_width = w;
        cell.bb_color = c;
    }
    Ok(())
}

/// 一条边 → `(style, width, color)`。
///
/// `style` **必填**（不猜默认线型）；`width` 缺省 `def::BORDER_WIDTH`（1.0）；
/// `color` 缺省 `def::COLOR_BLACK`。负宽度/负缩进一并拒。
fn parse_side(v: &J, path: &str) -> Result<(u8, f32, i32), DefError> {
    let o = obj(v, path)?;
    let sp = format!("{path}.style");
    let style = lookup_u8(BORDER_STYLES, as_str(field(o, "style", path)?, &sp)?, &sp)?;

    let width = match opt_f32(o, "width", path)? {
        None => def::BORDER_WIDTH,
        Some(w) if w < 0.0 => {
            return Err(err(
                &format!("{path}.width"),
                format!("线宽不得为负（实得 {w}）"),
            ))
        }
        Some(w) => w,
    };

    let cp = format!("{path}.color");
    let color = match o.get("color") {
        None => def::COLOR_BLACK,
        Some(c) => parse_color(as_str(c, &cp)?, &cp)?,
    };
    Ok((style, width, color))
}

/// 枚举查码（u8 形态：边框线型）。
fn lookup_u8(table: &[(&str, u8)], s: &str, path: &str) -> Result<u8, DefError> {
    table
        .iter()
        .find(|(name, _)| *name == s)
        .map(|(_, code)| *code)
        .ok_or_else(|| unknown_enum(table, s, path))
}

/// 枚举查码（i8 形态：`extend_mode`）。
fn lookup_i8(table: &[(&str, i8)], s: &str, path: &str) -> Result<i8, DefError> {
    table
        .iter()
        .find(|(name, _)| *name == s)
        .map(|(_, code)| *code)
        .ok_or_else(|| unknown_enum(table, s, path))
}

/// 未知枚举取值的错误（列出全部合法值 —— 不猜、不静默忽略）。
fn unknown_enum<T>(table: &[(&str, T)], s: &str, path: &str) -> DefError {
    let all = table
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(" / ");
    err(path, format!("未知取值 {s:?}（合法取值：{all}）"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reportdef::blank_cell;

    fn apply(json: &str) -> Cell {
        let o: Map<String, J> = serde_json::from_str(json).expect("测试 JSON 合法");
        let mut c = blank_cell();
        apply_cell_props(&mut c, &o, "$.cells[0]").expect("属性应被接受");
        c
    }

    /// 边框线型码**写死**（`NormalCell.java:1316-1336`）—— 期望值不从
    /// `BORDER_STYLES` 自己取，否则改码也照绿（M7-4 的同义反复教训）。
    #[test]
    fn border_styles_are_hardcoded_against_the_contract() {
        let expected: &[(&str, u8)] = &[
            ("none", 0),
            ("dotted", 81),
            ("dashed", 82),
            ("solid", 83),
            ("double", 84),
        ];
        assert_eq!(BORDER_STYLES.len(), expected.len(), "码表条目数变了");
        for (name, code) in expected {
            let c = apply(&format!(r#"{{"border":{{"left":{{"style":"{name}"}}}}}}"#));
            assert_eq!(c.lb_style, *code, "{name} → {code}");
        }
        // 未声明边的三槽保持默认（不得被同对象的另一边带偏）。
        let c = apply(r#"{"border":{"left":{"style":"solid"}}}"#);
        assert_eq!(c.rb_style, 0, "未声明右边 ⇒ style 默认 none");
        assert_eq!(c.tb_style, 0);
        assert_eq!(c.bb_style, 0);
    }

    /// 四条边互不干扰：只改自己那三槽。
    #[test]
    fn four_sides_land_on_their_own_slots() {
        let c = apply(
            // `r##`：JSON 里的颜色字面量含 `"#`，会撞上 `r#"..."#` 的收尾定界符。
            r##"{"border":{
                "left":   {"style":"solid",  "width":2,   "color":"#FF0000"},
                "right":  {"style":"dashed", "width":1.5, "color":"#00FF00"},
                "top":    {"style":"dotted", "width":0.5, "color":"#0000FF"},
                "bottom": {"style":"double"}
            }}"##,
        );
        assert_eq!(
            (c.lb_style, c.lb_width, c.lb_color),
            (83, 2.0, 0xFFFF0000u32 as i32)
        );
        assert_eq!(
            (c.rb_style, c.rb_width, c.rb_color),
            (82, 1.5, 0xFF00FF00u32 as i32)
        );
        assert_eq!(
            (c.tb_style, c.tb_width, c.tb_color),
            (81, 0.5, 0xFF0000FFu32 as i32)
        );
        // 只给 style ⇒ width/color 走契约默认。
        assert_eq!(
            (c.bb_style, c.bb_width, c.bb_color),
            (84, def::BORDER_WIDTH, def::COLOR_BLACK)
        );
    }

    /// 颜色与 `style.rs` 共用同一个解析器 ⇒ 同一套互逆保证。
    #[test]
    fn border_color_reuses_the_snapshot_reversible_parser() {
        let c = apply(r##"{"border":{"bottom":{"style":"solid","color":"#123ABC"}}}"##);
        assert_eq!(c.bb_color, 0xFF123ABCu32 as i32);
    }

    /// `extend_mode` 四态码写死（注 3；18/19 方向已由夹具实证）。
    #[test]
    fn extend_modes_are_hardcoded_against_the_contract() {
        let expected: &[(&str, i8)] = &[
            ("default", 16),
            ("none", 17),
            ("vertical", 18),
            ("horizontal", 19),
        ];
        for (name, code) in expected {
            let c = apply(&format!(r#"{{"extend_mode":"{name}"}}"#));
            assert_eq!(c.extend_mode, *code, "{name} → {code}");
        }
        // 缺省 ⇒ 默认 16（`blank_cell` 已给）。
        assert_eq!(apply("{}").extend_mode, def::EXTEND_DEFAULT);
    }

    /// `format` / `indent` / `rotation` 落槽；缺省保持默认。
    #[test]
    fn format_indent_rotation_land_on_their_slots() {
        let c = apply(r##"{"format":"#,##0.00","indent":2.5,"rotation":45}"##);
        assert_eq!(c.format.as_deref(), Some("#,##0.00"));
        assert_eq!(c.indent, 2.5);
        assert_eq!(c.rotation_angle, 45.0);

        let d = apply("{}");
        assert!(d.format.is_none(), "format 缺省 = None（不是空串）");
        assert_eq!(d.indent, def::INDENT);
        assert_eq!(d.rotation_angle, def::ROTATION_ANGLE);
    }

    /// 边界：负缩进 / 负旋转 / 负线宽 / 非数字 / 未知枚举 / 边内缺 style。
    #[test]
    fn boundaries_are_rejected_with_the_field_path() {
        let cases: &[(&str, &str)] = &[
            (r#"{"indent": -1}"#, "$.cells[0].indent"),
            (r#"{"rotation": -30}"#, "$.cells[0].rotation"),
            (
                r#"{"border":{"top":{"style":"solid","width":-2}}}"#,
                "$.cells[0].border.top.width",
            ),
            (r#"{"indent": "2"}"#, "$.cells[0].indent"),
            (r#"{"extend_mode":"diagonal"}"#, "$.cells[0].extend_mode"),
            (
                r#"{"border":{"left":{"width":1}}}"#,
                "$.cells[0].border.left.style",
            ),
            (
                r#"{"border":{"left":{"style":"wavy"}}}"#,
                "$.cells[0].border.left.style",
            ),
            (r#"{"border":"solid"}"#, "$.cells[0].border"),
        ];
        for (json, want) in cases {
            let o: Map<String, J> = serde_json::from_str(json).unwrap();
            let mut c = blank_cell();
            let e = apply_cell_props(&mut c, &o, "$.cells[0]").unwrap_err();
            let msg = format!("{e}");
            assert!(
                msg.starts_with(want),
                "{json} 的路径应为 {want}，实得 {msg}"
            );
        }
    }

    /// 未知枚举取值必须列出合法值（不猜、不静默忽略）。
    #[test]
    fn unknown_enum_lists_the_legal_values() {
        let o: Map<String, J> = serde_json::from_str(r#"{"extend_mode":"diagonal"}"#).unwrap();
        let mut c = blank_cell();
        let msg = format!("{}", apply_cell_props(&mut c, &o, "$.c").unwrap_err());
        assert!(
            msg.contains("default / none / vertical / horizontal"),
            "{msg}"
        );
    }

    /// 形态：`border` 的值不是对象时报路径（错误消息用 `json::kind_of` 的中文类型名）。
    #[test]
    fn border_shape_error_names_the_json_type() {
        let o: Map<String, J> = serde_json::from_str(r#"{"border": 1}"#).unwrap();
        let mut c = blank_cell();
        assert_eq!(
            format!("{}", apply_cell_props(&mut c, &o, "$.c").unwrap_err()),
            "$.c.border：应为对象，实得 数字"
        );
    }

    /// 只声明部分字段不得动其余槽（与 style.rs 同一纪律）。
    #[test]
    fn only_declared_fields_change() {
        let c = apply(r#"{"format":"0.00"}"#);
        assert_eq!(c.format.as_deref(), Some("0.00"));
        assert_eq!(c.lb_style, 0, "未声明边框 ⇒ 全边 none");
        assert_eq!(c.indent, def::INDENT);
        assert_eq!(c.rotation_angle, def::ROTATION_ANGLE);
        assert_eq!(c.extend_mode, def::EXTEND_DEFAULT);
        // 字体/颜色槽（style.rs 的地盘）也不得被本模块碰到。
        assert_eq!(c.font_size, def::FONT_SIZE);
        assert_eq!(c.fore_color, def::COLOR_BLACK);
    }

    /// 声明式主格：两槽独立落位，且**归一成大写**。
    ///
    /// 归一的意义：引擎的 `parse_address`（`heads/grid.rs:176-201`）内部转大写，
    /// 但模板里存的是**原串** —— 若这里不归一，`"b1"` 与 `"B1"` 在 `.rpx` 往返后
    /// 会变成两个不同字节串，roundtrip 判据会假红。
    #[test]
    fn declared_heads_land_on_their_slots_and_are_uppercased() {
        let c = apply(r#"{"left_head":"a1","top_head":"b2"}"#);
        assert_eq!(c.left_head.as_deref(), Some("A1"));
        assert_eq!(c.top_head.as_deref(), Some("B2"));

        // 缺省 = None（不是空串）。
        let d = apply("{}");
        assert!(d.left_head.is_none());
        assert!(d.top_head.is_none());

        // 只给一个不得带偏另一个 —— 两槽独立是交叉表的前提。
        let e = apply(r#"{"top_head":"C3"}"#);
        assert!(e.left_head.is_none());
        assert_eq!(e.top_head.as_deref(), Some("C3"));

        // 多字母列号（AA1 = 第 27 列）。
        assert_eq!(
            apply(r#"{"top_head":"aa1"}"#).top_head.as_deref(),
            Some("AA1")
        );
    }

    /// 形状非法必须报错并带路径；不接受显式根的别名。
    #[test]
    fn declared_head_shape_errors_are_loud() {
        let cases: &[(&str, &str)] = &[
            (r#"{"top_head":"B"}"#, "$.cells[0].top_head"), // 无数字段
            (r#"{"top_head":"1B"}"#, "$.cells[0].top_head"), // 数字在前
            (r#"{"top_head":"B1C"}"#, "$.cells[0].top_head"), // 字母在数字之后
            (r#"{"top_head":"0"}"#, "$.cells[0].top_head"), // 根标记必须带反引号
            (r#"{"top_head":""}"#, "$.cells[0].top_head"),  // 空串
            (r#"{"top_head":"$B$2"}"#, "$.cells[0].top_head"), // `$` 形态不在本契约内
            (
                r#"{"left_head":"B1","top_head":"zzzz1"}"#,
                "$.cells[0].top_head",
            ), // 4 字母越界
            (r#"{"top_head":2}"#, "$.cells[0].top_head"),   // 类型错
        ];
        for (json, want) in cases {
            let o: Map<String, J> = serde_json::from_str(json).unwrap();
            let mut c = blank_cell();
            let e = apply_cell_props(&mut c, &o, "$.cells[0]").unwrap_err();
            let msg = format!("{e}");
            assert!(
                msg.starts_with(want),
                "{json} 的路径应为 {want}，实得 {msg}"
            );
            // 失败不得留下半写状态（left_head 那例要求它仍是 None）。
            assert!(c.top_head.is_none(), "{json} 报错后 top_head 不应被写入");
        }
    }
    #[test]
    fn hyperlink_notes_and_window_are_optional_strings() {
        let c = apply(
            r#"{"hyperlink":"https://example.com","hyperlink_window":"_blank","notes":"备注"}"#,
        );
        assert_eq!(c.hyperlink.as_deref(), Some("https://example.com"));
        assert_eq!(c.hyperlink_window.as_deref(), Some("_blank"));
        assert_eq!(c.notes.as_deref(), Some("备注"));

        let o: Map<String, J> = serde_json::from_str(r#"{"hyperlink":""}"#).unwrap();
        let mut c = blank_cell();
        let e = apply_cell_props(&mut c, &o, "$.cells[0]").unwrap_err();
        assert!(format!("{e}").contains("超链接不能为空"));
        assert!(c.hyperlink.is_none());
    }
}
