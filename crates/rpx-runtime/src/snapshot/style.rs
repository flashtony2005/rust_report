//! 模板格样式 → Univer `IStyleData`（映射表：`docs/spec-m4.md` §4.3）。
//!
//! 映射字体、颜色、对齐与数字格式；数字原值保留供下载与校验使用。
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
    if let Some(pattern) = &cell.format {
        s.insert("n".to_string(), json!({ "pattern": pattern }));
    }
    // 边框（`spec-m4.md` §4.3 曾列为后置，2026-10-20 收口）：不输出的话结果页与
    // 打印预览都丢边框，只有设计视图（直接读定义 JSON）看得到。
    if let Some(bd) = border_json(cell) {
        s.insert("bd".to_string(), bd);
    }
    J::Object(s)
}

/// 四边边框 → Univer `IBorderData`（键 `t`/`r`/`b`/`l`，每边 `{ s, cl: { rgb } }`）。
///
/// 四边全 `none` ⇒ `None`（不写 `bd`，与「无边框」等价）。
fn border_json(cell: &Cell) -> Option<J> {
    let mut bd = Map::new();
    for (key, style, width, color) in [
        ("t", cell.tb_style, cell.tb_width, cell.tb_color),
        ("r", cell.rb_style, cell.rb_width, cell.rb_color),
        ("b", cell.bb_style, cell.bb_width, cell.bb_color),
        ("l", cell.lb_style, cell.lb_width, cell.lb_color),
    ] {
        let Some(code) = univer_border_code(style, width) else {
            continue;
        };
        bd.insert(
            key.to_string(),
            json!({ "s": code, "cl": { "rgb": argb_hex(color) } }),
        );
    }
    (!bd.is_empty()).then(|| J::Object(bd))
}

/// rpx 线型码 + 线宽 → Univer `BorderStyleTypes`（稠密枚举）。
///
/// rpx 只有 5 种线型（`format/modeled/naming.rs`：0 none / 81 dotted / 82 dashed /
/// 83 solid / 84 double），Univer 却有 14 档且**没有独立线宽字段** —— 宽度只能折进
/// 档位。rpx 默认线宽 `def::BORDER_WIDTH` = 1.0pt，故：`<1.25` 走细档、`<2.5` 走中档、
/// 再粗即 THICK（Univer 只有实线有 THICK，虚线最粗只到 MEDIUM_DASHED）。
fn univer_border_code(style: u8, width: f32) -> Option<i32> {
    let medium = width >= 1.25;
    let thick = width >= 2.5;
    Some(match style {
        81 => 3, // DOTTED
        82 => {
            if medium {
                9
            } else {
                4
            }
        } // MEDIUM_DASHED / DASHED
        83 => {
            if thick {
                13
            } else if medium {
                8
            } else {
                1
            }
        } // THICK / MEDIUM / THIN
        84 => 7, // DOUBLE
        _ => return None, // 0 或未知码：不猜
    })
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

#[cfg(test)]
mod tests {
    use super::univer_border_code;

    /// 线型 × 线宽 → Univer 档位。宽度是 rpx 唯一的粗细信息（Univer 无独立宽度槽），
    /// 折错档位等于把 1pt 实线画成 3pt 粗线，所以逐档钉死。
    #[test]
    fn border_width_folds_into_univer_style_code() {
        // 81 dotted 只有一档（Univer 无「粗点线」）
        assert_eq!(univer_border_code(81, 1.0), Some(3));
        assert_eq!(univer_border_code(81, 3.0), Some(3));
        // 82 dashed：细 → DASHED(4)，中 → MEDIUM_DASHED(9)
        assert_eq!(univer_border_code(82, 1.0), Some(4));
        assert_eq!(univer_border_code(82, 2.0), Some(9));
        // 83 solid：THIN(1) / MEDIUM(8) / THICK(13)
        assert_eq!(univer_border_code(83, 1.0), Some(1));
        assert_eq!(univer_border_code(83, 2.0), Some(8));
        assert_eq!(univer_border_code(83, 3.0), Some(13));
        // 84 double 只有一档
        assert_eq!(univer_border_code(84, 1.0), Some(7));
        // 0 = 无边框；未知码不猜（宁可不画，也不画错线型）
        assert_eq!(univer_border_code(0, 1.0), None);
        assert_eq!(univer_border_code(85, 1.0), None);
    }
}
