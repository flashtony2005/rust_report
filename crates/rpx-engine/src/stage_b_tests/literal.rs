//! 判据 3：字面量取值（`engine_types` 全网格）+ M2-14 类型保真的主对照。
//!
//! 之所以单独成文件：`stage_b_tests.rs` 已逼近 300 行门禁，而这两条判据与 head/改写
//! 判据的"未移动区"口径无关 —— 它们靠的是 `engine_types` 无扩展这一事实。

use super::json::{expected_text, fixture_value_token, parse_expected, template_dims};
use super::{cell_at, stage_b_for};
use crate::value::Value;

/// `engine_types` 是**纯字面量夹具**：`formulas: []`、模板 4×4、全 `extend=16`、
/// 无数据集、无扩展 ⇒ 计算网格 == 模板网格，于是 16 格可以**逐格全量**对拍
/// `value` + `type` + `disp_value`。
///
/// 这是 M2-14（类型保真）的唯一夹具级依据：`engine_types` 之前，所有夹具只产出过
/// `string`/`int`/`list` 三种类型（`_shape_audit.py`），"Integer 不得变 Double" 这条
/// 硬约束没有任何产品数据兜底。
#[test]
fn engine_types_literal_value_type_and_display_match_every_cell() {
    let sb = stage_b_for("engine_types");
    let text = expected_text("engine_types");
    let cells = parse_expected(&text);
    let (tr, tc) = template_dims(&text);
    // 夹具 `rows`/`cols` 是"不含 0 号脚手架"的口径（`CalcJson.java:40-42`），
    // 而 `StageB::rows` 是裸矩阵尺寸（含 0 号）—— 差 1 是口径差，不是错。
    assert_eq!(sb.rows - 1, tr, "计算网格行数（去 0 号）应等于模板行数（无扩展）");
    assert_eq!(sb.cols - 1, tc, "计算网格列数（去 0 号）应等于模板列数（无扩展）");
    assert_eq!(sb.expressed, 0, "engine_types 是纯字面量夹具，不应有表达式格");
    assert_eq!(sb.unmodeled_literals, 0, "engine_types 的 16 个字面量都必须是已建模变体");

    let mut checked = 0usize;
    for e in &cells {
        let cell = cell_at(&sb, e.r, e.c);
        let want_value = e
            .value
            .as_deref()
            .unwrap_or_else(|| panic!("夹具格 ({},{}) 缺 value", e.r, e.c));
        assert_eq!(
            fixture_value_token(&cell.value),
            want_value,
            "格 ({},{}) 的 value 不符",
            e.r,
            e.c
        );
        assert_eq!(
            cell.value.type_name(),
            e.ty.as_deref().expect("夹具格缺 type"),
            "格 ({},{}) 的 type 不符（类型保真 M2-14）",
            e.r,
            e.c
        );
        assert_eq!(cell.disp_value().as_deref(), e.disp.as_deref(), "格 ({},{}) 的 disp_value 不符", e.r, e.c);
        checked += 1;
    }
    assert_eq!(checked, 16, "engine_types 应逐格比对 16 格（模板 4×4）");
}

#[test]
fn engine_types_pins_the_m2_14_controls() {
    let sb = stage_b_for("engine_types");
    // (2,1) `Integer(100)`：M2-14 主对照 —— 不得被提升成 double
    assert_eq!(cell_at(&sb, 2, 1).value, Value::Int(100));
    assert_eq!(cell_at(&sb, 2, 1).value.type_name(), "int");
    // (2,4) `Double(100.0)`：JSON 原文被塌成 `100`，**只有 type 字段能区分**
    // —— 这条同时钉住"不得从 JSON 原文反推类型"。
    assert_eq!(cell_at(&sb, 2, 4).value, Value::Double(100.0));
    assert_eq!(cell_at(&sb, 2, 4).value.type_name(), "double");
    assert_eq!(fixture_value_token(&cell_at(&sb, 2, 4).value), "100");
    // (4,1) `BigDecimal("100.00")`：精度是信息，不得丢成 `100` 或 f64
    assert_eq!(cell_at(&sb, 4, 1).value, Value::Decimal("100.00".into()));
    assert_eq!(fixture_value_token(&cell_at(&sb, 4, 1).value), "100.00");
    // (1,1) 字符串 `"int"` 是**对照项**：内容是类型名也仍是 string，不得被 parse 成 int
    assert_eq!(cell_at(&sb, 1, 1).value, Value::Text("int".into()));
    assert_eq!(cell_at(&sb, 1, 1).value.type_name(), "string");
    // (3,2)/(3,3) 同理：`"boolean-true"` 不是 `true`
    assert_eq!(cell_at(&sb, 3, 2).value, Value::Text("boolean-true".into()));
    assert_eq!(cell_at(&sb, 4, 2).value, Value::Bool(true));
}
