//! 打印设置覆盖（定义 JSON 顶层 `print_setup` → [`ReportDefine::print_setup`]）。
//!
//! 落点是 `PrintSetup` 的 33 槽（`docs/format-contract.md` 附件 D）。**只开放其中
//! 语义明确的子集**，其余不开放 —— 理由见下，这是本文件最重要的一条纪律。
//!
//! # 为什么只开放一个子集
//!
//! 附件 D 给了全部 33 槽的**类型与默认值**，但**没有给码表**：`paper`、`orient`、
//! `pagerStyle`、`hAlign`、`vAlign`、`zoomMode`、`layout` 及各 `*_mode` 都是裸字节码，
//! 契约里只有默认值（`paper=9`、`orient=1`）。
//!
//! 夹具实证同样只有默认值：`tests/fixtures/` 里出现过的只有 `"paper":9` 与
//! `"orientation":1`，**没有任何夹具带横向版或其他纸张码**。⇒ 按本仓「不猜」纪律
//! （`parse.rs` 的模块文档），这些字段**不开放命名枚举**：
//! 给 `orient` 猜一个 `"landscape" → 2` 就是在没有任何证据的情况下定契约，
//! 而一旦猜错，前端写出的模板在 Java 侧会静默按错的版式渲染。
//!
//! 需要横竖版时**互换 `paper_width` / `paper_height`** —— 这两个是裸数值槽，
//! 语义无歧义，且分页器按它们算页宽页高（契约 §… 的 `engine_pager` A4 595×842pt）。
//!
//! 契约正文见 `docs/report-def-json.md` §3.6 / §4.7。

use serde_json::{Map, Value as J};

use rpx_core::print_setup::PrintSetup;

use super::json::{err, opt_bool, opt_f32, opt_str, opt_u64};
use super::DefError;

/// 开放的数值/布尔/字符串字段：`(JSON 名, 类型)`。
///
/// 数值分三档：`i16`（短）/ `i32`（整）/ `f32`（浮点）。取值范围**照格式的界**，
/// 不引入本契约自己的界（与 §4.5 字号的纪律一致）。
const I16_FIELDS: &[&str] = &[
    "zoom_scale",
    "table_column_num",
    "layout_rows",
    "layout_cols",
];

const I32_FIELDS: &[&str] = &["row_num_per_page", "copy_count"];

const F32_FIELDS: &[&str] = &[
    "paper_width",
    "paper_height",
    "left_margin",
    "right_margin",
    "top_margin",
    "bottom_margin",
];

const BOOL_FIELDS: &[&str] = &[
    "back_graph_printed",
    "column_repeated_group_header",
    "full_screen_preview",
    "blank_line_filled",
];

/// **已知但不开放**的字段：附件 D 有这些槽，但契约没给码表、夹具也只见过默认值
/// ⇒ 按「不猜」纪律不设命名枚举。列出来是为了给**精确的错误消息**（「未开放」
/// 与「字段名拼错」是两种不同的错，前端排错方式不同）。
const KNOWN_UNEXPOSED: &[&str] = &[
    "paper",
    "orient",
    "orientation",
    "h_align",
    "v_align",
    "zoom_mode",
    "layout",
    "page_header_and_footer_y_mode",
    "page_header_and_footer_x_mode",
    "row_table_header_and_footer_mode",
    "title_y_mode",
    "title_x_mode",
    "col_table_header_and_footer_mode",
    "column_mode",
    "zoom_page_width",
    "zoom_page_height",
];

/// 全部**开放**字段（用于「未知字段显式拒绝」）。
fn exposed_fields() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    v.extend_from_slice(I16_FIELDS);
    v.extend_from_slice(I32_FIELDS);
    v.extend_from_slice(F32_FIELDS);
    v.extend_from_slice(BOOL_FIELDS);
    v.extend(["virtual_printer", "pager_style"]);
    v
}

/// 顶层 `print_setup` → [`PrintSetup`]（缺省 ⇒ 契约默认值，不是错误）。
///
/// **逐字段覆盖**：给了才改，不给保持 `PrintSetup::default_values()` ——
/// 与 `apply_style` / `apply_cell_props` 同一纪律。
pub(super) fn apply_print_setup(
    ps: &mut PrintSetup,
    root: &Map<String, J>,
) -> Result<(), DefError> {
    let Some(v) = root.get("print_setup") else {
        return Ok(());
    };
    let path = "$.print_setup";
    let o = super::json::obj(v, path)?;

    // 先拒未知/未开放字段，再落位 —— 否则「设了 `orient` 却没生效」会静默通过。
    // 这是本仓反复踩过的失败形态（`parse.rs` 模块文档的「不猜」纪律）。
    for key in o.keys() {
        if KNOWN_UNEXPOSED.contains(&key.as_str()) {
            return Err(err(
                &format!("{path}.{key}"),
                "未开放：契约附件 D 只给了该槽的类型与默认值、未给码表，\
                 夹具实证也只有默认值 ⇒ 按「不猜」纪律不设命名枚举",
            ));
        }
        if !exposed_fields().contains(&key.as_str()) {
            return Err(err(
                &format!("{path}.{key}"),
                format!("未知字段（开放字段：{}）", exposed_fields().join(" / ")),
            ));
        }
    }

    if let Some(mode) = opt_str(o, "pager_style", path)? {
        ps.pager_style = match mode {
            "none" => 0,
            "rows" => 1,
            "paper" => 2,
            _ => return Err(err("$.print_setup.pager_style", "可选 none / rows / paper")),
        };
    }
    if o.contains_key("paper_width") || o.contains_key("paper_height") {
        ps.paper = 256;
    }
    for name in I16_FIELDS {
        if let Some(n) = opt_u64(o, name, path)? {
            let p = format!("{path}.{name}");
            if n > i16::MAX as u64 {
                return Err(err(
                    &p,
                    format!("{name} 越界（0..={}，即格式的 i16 上限）", i16::MAX),
                ));
            }
            assign_i16(ps, name, n as i16);
        }
    }
    for name in I32_FIELDS {
        if *name == "row_num_per_page" && o.get(*name).is_some_and(J::is_string) {
            continue;
        }
        if let Some(n) = opt_u64(o, name, path)? {
            let p = format!("{path}.{name}");
            if n > i32::MAX as u64 {
                return Err(err(
                    &p,
                    format!("{name} 越界（0..={}，即格式的 i32 上限）", i32::MAX),
                ));
            }
            assign_i32(ps, name, n as i32);
        }
    }
    for name in F32_FIELDS {
        if let Some(f) = opt_f32(o, name, path)? {
            let p = format!("{path}.{name}");
            if f < 0.0 {
                return Err(err(&p, format!("{name} 不得为负（实得 {f}）")));
            }
            assign_f32(ps, name, f);
        }
    }
    for name in BOOL_FIELDS {
        if let Some(b) = opt_bool(o, name, path)? {
            assign_bool(ps, name, b);
        }
    }
    if let Some(vp) = opt_str(o, "virtual_printer", path)? {
        ps.virtual_printer = Some(vp.to_string());
    }
    if ps.pager_style == 1 && ps.row_num_per_page <= 0 {
        return Err(err(
            "$.print_setup.row_num_per_page",
            "按行分页的每页行数必须大于 0",
        ));
    }
    Ok(())
}

/// i16 字段落位（`PrintSetup` 的三个短整字段 + `zoom_scale`）。
fn assign_i16(ps: &mut PrintSetup, name: &str, v: i16) {
    match name {
        "zoom_scale" => ps.zoom_scale = v,
        "table_column_num" => ps.table_column_num = v,
        "layout_rows" => ps.layout_rows = v,
        "layout_cols" => ps.layout_cols = v,
        _ => unreachable!("I16_FIELDS 与 assign_i16 不同步"),
    }
}

/// i32 字段落位。
fn assign_i32(ps: &mut PrintSetup, name: &str, v: i32) {
    match name {
        "row_num_per_page" => ps.row_num_per_page = v,
        "copy_count" => ps.copy_count = v,
        _ => unreachable!("I32_FIELDS 与 assign_i32 不同步"),
    }
}

/// f32 字段落位（纸张尺寸与四边距）。
fn assign_f32(ps: &mut PrintSetup, name: &str, v: f32) {
    match name {
        "paper_width" => ps.paper_width = v,
        "paper_height" => ps.paper_height = v,
        "left_margin" => ps.left_margin = v,
        "right_margin" => ps.right_margin = v,
        "top_margin" => ps.top_margin = v,
        "bottom_margin" => ps.bottom_margin = v,
        _ => unreachable!("F32_FIELDS 与 assign_f32 不同步"),
    }
}
/// 布尔字段落位。
fn assign_bool(ps: &mut PrintSetup, name: &str, v: bool) {
    match name {
        "back_graph_printed" => ps.back_graph_printed = v,
        "column_repeated_group_header" => ps.column_repeated_group_header = v,
        "full_screen_preview" => ps.full_screen_preview = v,
        "blank_line_filled" => ps.blank_line_filled = v,
        _ => unreachable!("BOOL_FIELDS 与 assign_bool 不同步"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpx_core::print_setup::PrintSetup;

    fn apply(root: &str) -> PrintSetup {
        let r: Map<String, J> = serde_json::from_str(root).expect("测试 JSON 合法");
        let mut ps = PrintSetup::default_values();
        apply_print_setup(&mut ps, &r).expect("打印设置应被接受");
        ps
    }

    /// 缺省 ⇒ 契约默认值一字不动（`default_values()` 是唯一事实源）。
    #[test]
    fn absent_print_setup_keeps_every_contract_default() {
        let ps = apply(r#"{"grid":{"rows":1,"cols":1}}"#);
        assert_eq!(ps, PrintSetup::default_values());
        let ps = apply(r#"{"print_setup":{}}"#);
        assert_eq!(ps, PrintSetup::default_values());
    }

    /// 逐字段覆盖：只声明两个字段，其余 31 个槽仍是默认。
    #[test]
    fn only_declared_fields_change() {
        let ps = apply(r#"{"print_setup":{"row_num_per_page":40,"zoom_scale":80}}"#);
        assert_eq!(ps.row_num_per_page, 40);
        assert_eq!(ps.zoom_scale, 80);
        assert_eq!(ps.paper_width, 210.0, "未声明纸张尺寸 ⇒ 默认 A4 宽");
        assert_eq!(ps.paper_height, 297.0);
        assert_eq!(ps.left_margin, 19.0);
        assert_eq!(ps.copy_count, 1);
        assert!(!ps.full_screen_preview);
    }

    /// 纸张尺寸 + 四边距全部落位（横向版 = 宽高互换）。
    #[test]
    fn paper_and_margins_land_on_their_slots() {
        let ps = apply(
            r#"{"print_setup":{
                "paper_width": 297, "paper_height": 210,
                "left_margin": 10, "right_margin": 12,
                "top_margin": 15, "bottom_margin": 18
            }}"#,
        );
        assert_eq!((ps.paper_width, ps.paper_height), (297.0, 210.0), "横向版");
        assert_eq!((ps.left_margin, ps.right_margin), (10.0, 12.0));
        assert_eq!((ps.top_margin, ps.bottom_margin), (15.0, 18.0));
    }

    /// 四个布尔槽逐个落位。
    #[test]
    fn boolean_slots_land() {
        let ps = apply(
            r#"{"print_setup":{
                "back_graph_printed": true,
                "column_repeated_group_header": true,
                "full_screen_preview": true,
                "blank_line_filled": true
            }}"#,
        );
        assert!(ps.back_graph_printed);
        assert!(ps.column_repeated_group_header);
        assert!(ps.full_screen_preview);
        assert!(ps.blank_line_filled);
    }

    /// `virtual_printer` 字符串槽。
    #[test]
    fn virtual_printer_slot_lands() {
        let ps = apply(r#"{"print_setup":{"virtual_printer":"Microsoft Print to PDF"}}"#);
        assert_eq!(
            ps.virtual_printer.as_deref(),
            Some("Microsoft Print to PDF")
        );
    }

    /// 越界与形态错误一律带 JSON 路径（界 = 格式的界，不另立）。
    #[test]
    fn boundaries_carry_the_field_path() {
        let cases: &[(&str, &str)] = &[
            (
                r#"{"print_setup":{"zoom_scale":40000}}"#,
                "$.print_setup.zoom_scale",
            ),
            (
                r#"{"print_setup":{"copy_count":-1}}"#,
                "$.print_setup.copy_count",
            ),
            (
                r#"{"print_setup":{"paper_width":-210}}"#,
                "$.print_setup.paper_width",
            ),
            (
                r#"{"print_setup":{"left_margin":"19"}}"#,
                "$.print_setup.left_margin",
            ),
            (
                r#"{"print_setup":{"full_screen_preview":1}}"#,
                "$.print_setup.full_screen_preview",
            ),
            (r#"{"print_setup":"A4"}"#, "$.print_setup"),
        ];
        for (root, want) in cases {
            let r: Map<String, J> = serde_json::from_str(root).unwrap();
            let mut ps = PrintSetup::default_values();
            let e = apply_print_setup(&mut ps, &r).unwrap_err();
            let msg = format!("{e}");
            assert!(
                msg.starts_with(want),
                "{root} 的路径应为 {want}，实得 {msg}"
            );
        }
    }

    /// **不开放的字段**（`paper` / `orient` / `pager_style` / `h_align` / `v_align` /
    /// `zoom_mode` / `layout` / 各 `*_mode`）不得被静默接受后丢弃 ——
    /// 那正是「前端以为设了、实际没生效」的失败形态。**显式报错**。
    #[test]
    fn unexposed_fields_are_refused_loudly() {
        for name in [
            "paper",
            "orient",
            "orientation",
            "h_align",
            "v_align",
            "zoom_mode",
            "layout",
            "page_header_and_footer_y_mode",
            "title_x_mode",
            "column_mode",
        ] {
            let root = format!(r#"{{"print_setup":{{"{name}":1}}}}"#);
            let r: Map<String, J> = serde_json::from_str(&root).unwrap();
            let mut ps = PrintSetup::default_values();
            let e = apply_print_setup(&mut ps, &r).unwrap_err();
            let msg = format!("{e}");
            assert!(
                msg.starts_with(&format!("$.print_setup.{name}")),
                "{name} 未被显式拒绝，实得 {msg}"
            );
            assert!(msg.contains("未开放"), "错误消息应说明原因：{msg}");
        }
    }
}
