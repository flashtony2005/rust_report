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
    let c =
        apply(r#"{"hyperlink":"https://example.com","hyperlink_window":"_blank","notes":"备注"}"#);
    assert_eq!(c.hyperlink.as_deref(), Some("https://example.com"));
    assert_eq!(c.hyperlink_window.as_deref(), Some("_blank"));
    assert_eq!(c.notes.as_deref(), Some("备注"));

    let o: Map<String, J> = serde_json::from_str(r#"{"hyperlink":""}"#).unwrap();
    let mut c = blank_cell();
    let e = apply_cell_props(&mut c, &o, "$.cells[0]").unwrap_err();
    assert!(format!("{e}").contains("超链接不能为空"));
    assert!(c.hyperlink.is_none());
}
#[test]
fn diagonal_uses_the_border_style_table() {
    let c = apply(r##"{"diagonal":{"style":"dashed","width":2,"color":"#123456"}}"##);
    assert_eq!(
        (c.diagonal_style, c.diagonal_width, c.diagonal_color),
        (82, 2.0, 0xFF12_3456u32 as i32)
    );
}
