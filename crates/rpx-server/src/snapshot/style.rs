//! 模板格样式 → Univer `IStyleData`（映射表：`docs/spec-m4.md` §4.3）。
//!
//! v1 只做 9 个字段（`bl`/`it`/`ul`/`ff`/`fs`/`cl`/`bg`/`ht`/`vt`）；
//! 边框 / 数字格式 / 旋转 / 缩进后置（同表末行）。
//!
//! **对齐总是输出**：rpx 默认垂直居中（`def::VALIGN_MIDDLE`），而 Univer 的默认
//! 与之不同 ⇒ 省略即视觉漂移，必须显式。

use serde_json::{json, Map, Value as J};

use rpx_core::cell::Cell;
use rpx_core::props::def;

/// 一格的内联 `IStyleData`（v1 内联、不建样式表：`spec-m4.md` §4.1）。
pub(crate) fn style_json(cell: &Cell) -> J {
    let mut s = Map::new();
    // 字体（默认"宋体"12 —— `def::DEFAULT_FONT_NAME` / `def::FONT_SIZE`）
    s.insert(
        "ff".to_string(),
        json!(cell.font_name.as_deref().unwrap_or(def::DEFAULT_FONT_NAME)),
    );
    s.insert("fs".to_string(), json!(cell.font_size));
    // 位标志（`props::bit` 的位打包，附件 B.3.1）
    if cell.bold() {
        s.insert("bl".to_string(), json!(1));
    }
    if cell.italic() {
        s.insert("it".to_string(), json!(1));
    }
    if cell.underline() {
        // `ITextDecoration`：{s:1} = 单下划线（线型细分后置）
        s.insert("ul".to_string(), json!({ "s": 1 }));
    }
    // 颜色：ARGB int → "#RRGGBB"（`def::COLOR_BLACK` 0xFF000000 → #000000）
    s.insert(
        "cl".to_string(),
        json!({ "rgb": argb_hex(cell.fore_color) }),
    );
    s.insert(
        "bg".to_string(),
        json!({ "rgb": argb_hex(cell.back_color) }),
    );
    // 对齐（数字枚举；取值表见下两函数）
    if let Some(ht) = h_align_code(cell.h_align) {
        s.insert("ht".to_string(), json!(ht));
    }
    if let Some(vt) = v_align_code(cell.v_align) {
        s.insert("vt".to_string(), json!(vt));
    }
    J::Object(s)
}

/// ARGB int（`0xAARRGGBB`）→ `#RRGGBB`（低 24 位）。
pub(crate) fn argb_hex(c: i32) -> String {
    format!("#{:06X}", (c as u32) & 0x00FF_FFFF)
}

/// `h_align` → Univer `HorizontalAlign`（`format-contract.md` 注 3 × Univer 官方枚举）：
/// `-48`=left→1、`-47`=center→2、`-46`=right→3、`-45`=justified→4、`-44`=distributed→6。
/// 未知值 → `None`（不猜；Univer 落默认）。
pub(crate) fn h_align_code(v: i8) -> Option<i32> {
    match v {
        -48 => Some(1),
        -47 => Some(2),
        -46 => Some(3),
        -45 => Some(4),
        -44 => Some(6),
        _ => None,
    }
}

/// `v_align` → Univer `VerticalAlign`：`-32`=top→1、`-31`=middle→2、`-30`=bottom→3。
pub(crate) fn v_align_code(v: i8) -> Option<i32> {
    match v {
        -32 => Some(1),
        -31 => Some(2),
        -30 => Some(3),
        _ => None,
    }
}
