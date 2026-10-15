//! 定义 JSON 的**模型往返**判据（M7-5 / M7-6）。
//!
//! 为什么不是快照读回：`snapshot::style::style_json` 只暴露 9 个字段
//! （`bl/it/ul/ff/fs/cl/bg/ht/vt`），**边框 / 数字格式 / 缩进 / 旋转 / 打印设置不在
//! 其中**（`spec-m4.md` §4.3 明确列为后置）。所以端到端 HTTP 读回走不通。
//!
//! 判据改为经**真实 `.rpx` 字节**闭合整圈：
//!
//! ```text
//! JSON ──parse──▶ ReportDefine ──to_bytes──▶ .rpx 字节 ──open_bytes──▶ ReportDefine'
//! ```
//!
//! 断言 `ReportDefine' == ReportDefine`。这一条同时证明三件事：
//! ① `cellprops.rs` / `printsetup.rs` 把 JSON 写对了槽；② `write_cell` /
//! `write_print_setup`（M7-2 写侧）按附件 B.3.2 / D 的槽序写对；③ 读侧按同一槽序
//! 读回。**任何一环错位都会红**。
//!
//! ⚠ **往返相等抓不到「两边一致地错」**：把 `dashed` 的码从 82 改成 83，
//! 往返照样绿（解析写 83、读侧读回 83）。所以**另有一条写死期望值的判据**
//! （`cell_props_land_on_the_expected_slots` 等）钉住每个槽的具体取值 ——
//! 两类判据互补，缺一不可（已用故障注入实证）。
//!
//! 这是 M7-2 在 `rpx-core/tests/roundtrip.rs` 立下的判据形态，此处沿用到 JSON 侧。

use rpx_core::{open_bytes, to_bytes, ReportDefine};
use rpx_server::reportdef;

/// 覆盖全部 16 个新槽（边框 4 边 × 3 + format + indent + rotation + extend_mode），
/// 外加字体字号颜色（M7-3 的地盘）与合并（M7-1 的地盘）—— 一次把三代增量的槽
/// 都压进同一条往返链，防止后来的改动踩坏前面的。
const DEF_FULL: &str = r##"{
  "version": 1,
  "grid": { "rows": 2, "cols": 2 },
  "cells": [
    { "row": 1, "col": 1, "text": "全属性",
      "font_name": "微软雅黑", "font_size": 14, "bold": true,
      "fore_color": "#FF0000", "back_color": "#FFFFFF",
      "format": "#,##0.00", "indent": 2.5, "rotation": 45,
      "extend_mode": "vertical",
      "border": {
        "left":   {"style":"solid",  "width":2,   "color":"#FF0000"},
        "right":  {"style":"dashed", "width":1.5, "color":"#00FF00"},
        "top":    {"style":"dotted", "width":0.5, "color":"#0000FF"},
        "bottom": {"style":"double"}
      } },
    { "row": 1, "col": 2, "text": "单边", "border": { "top": {"style":"double"} } },
    { "row": 2, "col": 1, "text": "默认", "extend_mode": "none" }
  ]
}"##;

/// 解析 → 写字节 → 读回：整圈必须逐字段相等。
fn round_trip(def: &str) -> ReportDefine {
    let rd = reportdef::build(def).expect("定义应被接受");
    let bytes = to_bytes(&rd).expect("写侧应成功");
    open_bytes(&bytes).expect("读回应成功")
}

#[test]
fn cell_props_survive_the_rpx_round_trip() {
    let rd = reportdef::build(DEF_FULL).expect("定义应被接受");
    let back = round_trip(DEF_FULL);
    assert_eq!(back, rd, "16 个属性槽经 .rpx 字节往返后必须逐字段相等");
}

/// 逐槽断言（往返相等之外，再钉住每个槽的**具体取值**——
/// 否则「两边都写错成同一个值」也能让 `==` 通过）。
#[test]
fn cell_props_land_on_the_expected_slots() {
    let rd = reportdef::build(DEF_FULL).expect("定义应被接受");
    let full = rd.cell_at(1, 1).expect("(1,1) 应有模板格");

    // 边框四边：style / width / color 各归其位。
    assert_eq!(
        (full.lb_style, full.lb_width, full.lb_color),
        (83, 2.0, 0xFFFF0000u32 as i32),
        "left"
    );
    assert_eq!(
        (full.rb_style, full.rb_width, full.rb_color),
        (82, 1.5, 0xFF00FF00u32 as i32),
        "right"
    );
    assert_eq!(
        (full.tb_style, full.tb_width, full.tb_color),
        (81, 0.5, 0xFF0000FFu32 as i32),
        "top"
    );
    assert_eq!(
        (full.bb_style, full.bb_width, full.bb_color),
        (84, 1.0, 0xFF000000u32 as i32),
        "bottom：只给 style ⇒ width/color 走默认"
    );
    // 数字格式 / 缩进 / 旋转 / 扩展模式。
    assert_eq!(full.format.as_deref(), Some("#,##0.00"));
    assert_eq!(full.indent, 2.5);
    assert_eq!(full.rotation_angle, 45.0);
    assert_eq!(full.extend_mode, 18, "vertical = EXTEND_VERTICAL");
    // M7-3 的槽不得被本模块碰坏。
    assert_eq!(full.font_name.as_deref(), Some("微软雅黑"));
    assert_eq!(full.font_size, 14);
    assert!(full.bold());
    assert_eq!(full.fore_color, 0xFFFF0000u32 as i32);

    // 只声明一条边 ⇒ 其余三边保持默认。
    let single = rd.cell_at(1, 2).expect("(1,2) 应有模板格");
    assert_eq!(single.tb_style, 84, "top = double");
    assert_eq!(
        (single.lb_style, single.rb_style, single.bb_style),
        (0, 0, 0)
    );
    assert_eq!(single.tb_width, 1.0, "未给 width ⇒ 默认 1.0");
    assert_eq!(single.tb_color, 0xFF000000u32 as i32, "未给 color ⇒ 默认黑");

    // 完全不声明属性 ⇒ 全部默认（含 extend_mode 的 16）。
    let plain = rd.cell_at(2, 1).expect("(2,1) 应有模板格");
    assert_eq!(plain.extend_mode, 17, "none = EXTEND_NONE");
    assert!(plain.format.is_none());
    assert_eq!(
        (
            plain.lb_style,
            plain.rb_style,
            plain.tb_style,
            plain.bb_style
        ),
        (0, 0, 0, 0)
    );
    assert_eq!((plain.indent, plain.rotation_angle), (0.0, 0.0));
}

/// 往返必须产生**合法 `.rpx` 容器**（魔数 + 长度 + 尾注）—— 顺带再压一层
/// 文件层封装（M7-2 的 `to_bytes`）。
#[test]
fn round_trip_produces_a_well_formed_container() {
    let rd = reportdef::build(DEF_FULL).unwrap();
    let bytes = to_bytes(&rd).unwrap();
    assert!(bytes.starts_with(b"RQQR"), "应带魔数");
    let n = i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(bytes.len() as i64, i64::from(n) + 16, "长度头应自洽");
    assert_eq!(
        &bytes[bytes.len() - 8..],
        &[82, 81, 81, 82, 82, 81, 81, 82],
        "尾注"
    );
}

// ---------------------------------------------------------------------------
// 打印设置覆盖（M7-6）
// ---------------------------------------------------------------------------

/// 覆盖全部开放字段（含横向版的宽高互换 + 四边距 + 两个 i32 + 四个布尔）。
const DEF_PRINT: &str = r#"{
  "version": 1,
  "grid": { "rows": 2, "cols": 2 },
  "cells": [ { "row": 1, "col": 1, "text": "横版明细" } ],
  "print_setup": {
    "paper_width": 297, "paper_height": 210,
    "left_margin": 10, "right_margin": 12, "top_margin": 15, "bottom_margin": 18,
    "row_num_per_page": 40, "copy_count": 2,
    "zoom_scale": 80, "table_column_num": 2, "layout_rows": 1, "layout_cols": 1,
    "back_graph_printed": true, "blank_line_filled": true,
    "virtual_printer": "Microsoft Print to PDF"
  }
}"#;

/// 打印设置经 `.rpx` 字节往返后逐字段相等。
#[test]
fn print_setup_survives_the_rpx_round_trip() {
    let rd = reportdef::build(DEF_PRINT).expect("定义应被接受");
    let back = round_trip(DEF_PRINT);
    assert_eq!(
        back.print_setup, rd.print_setup,
        "打印设置往返后必须逐字段相等"
    );
}

/// 打印设置逐槽断言（写死期望值，与单元格属性的理由相同）。
#[test]
fn print_setup_lands_on_the_expected_slots() {
    let ps = reportdef::build(DEF_PRINT).unwrap().print_setup;
    assert_eq!(
        (ps.paper_width, ps.paper_height),
        (297.0, 210.0),
        "横向版 = 宽高互换"
    );
    assert_eq!((ps.left_margin, ps.right_margin), (10.0, 12.0));
    assert_eq!((ps.top_margin, ps.bottom_margin), (15.0, 18.0));
    assert_eq!(ps.row_num_per_page, 40);
    assert_eq!(ps.copy_count, 2);
    assert_eq!(ps.zoom_scale, 80);
    assert_eq!(ps.table_column_num, 2);
    assert_eq!((ps.layout_rows, ps.layout_cols), (1, 1));
    assert!(ps.back_graph_printed);
    assert!(ps.blank_line_filled);
    assert_eq!(
        ps.virtual_printer.as_deref(),
        Some("Microsoft Print to PDF")
    );
    // 未声明的槽保持契约默认（不得被同对象的其它字段带偏）。
    assert!(!ps.full_screen_preview);
    assert!(!ps.column_repeated_group_header);
}

/// **不开放的字段不得被静默接受**（否则「前端以为设了 orient、实际没生效」）。
#[test]
fn unexposed_print_setup_fields_are_refused_loudly() {
    for name in [
        "paper",
        "orient",
        "orientation",
        "pager_style",
        "h_align",
        "column_mode",
    ] {
        let def =
            format!(r#"{{"version":1,"grid":{{"rows":1,"cols":1}},"print_setup":{{"{name}":1}}}}"#);
        let e = reportdef::build(&def).unwrap_err();
        let msg = format!("{e}");
        assert!(
            msg.starts_with(&format!("$.print_setup.{name}")),
            "{name} 未被显式拒绝，实得 {msg}"
        );
        assert!(msg.contains("未开放"), "错误消息应说明原因：{msg}");
    }
}
