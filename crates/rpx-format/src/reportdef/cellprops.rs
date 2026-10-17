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
    if let Some(d) = o.get("diagonal") {
        let dp = format!("{path}.diagonal");
        let (style, width, color) = parse_side(d, &dp)?;
        cell.diagonal_style = style;
        cell.diagonal_width = width;
        cell.diagonal_color = color;
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
mod tests;
