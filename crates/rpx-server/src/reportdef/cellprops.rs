//! 单元格属性扩展（定义 JSON → 模板格）：**边框 / 数字格式 / 缩进 / 旋转 / 扩展模式**。
//!
//! 与 [`super::style`]（字体字号颜色粗斜下划线）互补 —— 那边 7 个槽，这边 16 个：
//! 边框 12（4 边 × style/width/color）+ `format` + `indent` + `rotation_angle` +
//! `extend_mode`。
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
}
