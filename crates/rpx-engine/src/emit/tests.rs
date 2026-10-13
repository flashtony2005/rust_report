//! `emit` 的单测：把"没有夹具钉住"的行为（尤其是 Java 数字记法）显式钉住。
//!
//! 12 个引擎夹具里只有 `engine_types` 带 `double`/`float`/`decimal` 三格，
//! 且那两个 `double` 一个走整数折叠（`100`）、一个走 `Double.toString(92.5)`，
//! **没有**任何一格落在科学记数法或 `Float.toString` 的边界上。所以本文件是这些
//! 分支的唯一护栏；数字记法的期望值全部来自 `Double.toString`/`Float.toString`
//! 的 javadoc 语义（见 `num.rs` 模块文档），不是从夹具抄的。
//!
//! ⚠ 期望值里的长字符串用 Rust 的 `\` 续行书写：Rust 会吃掉换行**以及下一行的
//! 全部前导空白**，所以缩进不影响比较对象。

use super::json;
use super::num;
use super::val;
use super::{render_calculated, render_calculated_error};
use crate::arena::{CalcCell, CalcGrid, SourceRef, SubValue};
use crate::extend::ExtendMode;
use crate::value::Value;

// =====================================================================
// Java 数字记法
// =====================================================================

#[test]
fn double_to_string_switches_notation_at_java_thresholds() {
    // 记法切换点：`-3 <= exp < 7` 用位置记法（javadoc 语义 + num.rs 模块文档）
    assert_eq!(num::java_double_to_string(1e7), "1.0E7"); // exp = 7，不含端点 → 科学
    assert_eq!(num::java_double_to_string(1234567.0), "1234567.0"); // exp = 6 → 位置
    assert_eq!(num::java_double_to_string(1e-3), "0.001"); // exp = -3，含端点 → 位置
    assert_eq!(num::java_double_to_string(1e-4), "1.0E-4"); // exp = -4 → 科学
    assert_eq!(num::java_double_to_string(1e21), "1.0E21");
    assert_eq!(num::java_double_to_string(1e300), "1.0E300");
    // 整数尾数必须带 `.0`（Rust 的 Display 会给 "100"，那是错的）
    assert_eq!(num::java_double_to_string(100.0), "100.0");
}

#[test]
fn double_to_string_keeps_the_shortest_round_trip_digits() {
    // 这两条的期望值就是 Java 的 Double.toString（javadoc 保证"最短且唯一确定"）
    assert_eq!(num::java_double_to_string(0.1 + 0.2), "0.30000000000000004");
    assert_eq!(num::java_double_to_string(1.0 / 3.0), "0.3333333333333333");
    assert_eq!(num::java_double_to_string(92.5), "92.5");
}

#[test]
fn double_to_string_handles_signs_and_specials() {
    // 负零与正零分开（Java Double.toString(-0.0) == "-0.0"）
    assert_eq!(num::java_double_to_string(-0.0), "-0.0");
    assert_eq!(num::java_double_to_string(0.0), "0.0");
    assert_eq!(num::java_double_to_string(f64::NAN), "NaN");
    assert_eq!(num::java_double_to_string(f64::INFINITY), "Infinity");
    assert_eq!(num::java_double_to_string(f64::NEG_INFINITY), "-Infinity");
    assert_eq!(num::java_double_to_string(-1234.5), "-1234.5");
}

#[test]
fn float_to_string_uses_float_precision_not_double() {
    // Float.toString 的有效位数按 f32；`0.1f` 的最短串是 "0.1"（不是 f64 的那一串）
    assert_eq!(num::java_float_to_string(1.5), "1.5");
    assert_eq!(num::java_float_to_string(0.1), "0.1");
    assert_eq!(num::java_float_to_string(1e7), "1.0E7");
    assert_eq!(num::java_float_to_string(f32::NAN), "NaN");
    assert_eq!(num::java_float_to_string(-0.0), "-0.0");
}

// =====================================================================
// JSON 转义
// =====================================================================

#[test]
fn quote_escapes_exactly_like_json_java() {
    assert_eq!(json::quote("abc"), "\"abc\"");
    assert_eq!(json::quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    assert_eq!(json::quote("l1\nl2\tt\r"), "\"l1\\nl2\\tt\\r\"");
    // `Json.java:135` 的判据是 `c < 0x20 || c > 0x7E` ⇒ 中文必须转义（夹具原文如此）
    assert_eq!(json::quote("仅显示值"), "\"\\u4ec5\\u663e\\u793a\\u503c\"");
    assert_eq!(json::quote("\u{1}"), "\"\\u0001\"");
    // BMP 外字符按 UTF-16 码元拆成两个 \uXXXX（Java 的 char 是 16 位）
    assert_eq!(json::quote("\u{1F600}"), "\"\\ud83d\\ude00\"");
}

// =====================================================================
// 值渲染
// =====================================================================

#[test]
fn render_value_mirrors_calcjson_value() {
    assert_eq!(val::render_value(&Value::Null), "null");
    assert_eq!(val::render_value(&Value::Text("N\"orth".into())), "\"N\\\"orth\"");
    assert_eq!(val::render_value(&Value::Bool(true)), "true");
    assert_eq!(val::render_value(&Value::Int(100)), "100");
    assert_eq!(val::render_value(&Value::Long(1234567890123)), "1234567890123");
    // BigDecimal 原样搬（`100.0` 的尾零是精度信息，不许重排成 `100`）
    assert_eq!(val::render_value(&Value::Decimal("100.00".into())), "100.00");
    // 整数值的 Double 被折叠成裸整数（CalcJson.java:241-243）
    assert_eq!(val::render_value(&Value::Double(100.0)), "100");
    assert_eq!(val::render_value(&Value::Double(92.5)), "92.5");
    assert_eq!(val::render_value(&Value::Float(1.5)), "1.5");
    // NaN / Inf 走**带引号**字符串，且用的是 double 的写法
    assert_eq!(val::render_value(&Value::Double(f64::NAN)), "\"NaN\"");
    assert_eq!(val::render_value(&Value::Float(f32::INFINITY)), "\"Infinity\"");
    assert_eq!(
        val::render_value(&Value::List(vec![Value::Text("R1".into()), Value::Int(2)])),
        "[\"R1\",2]"
    );
}

#[test]
fn java_tostring_differs_from_render_value_for_integral_doubles() {
    // 这条差异是 disp_value 判定的关键：`String.valueOf(Double(100.0))` == "100.0"
    assert_eq!(val::java_tostring(&Value::Double(100.0)), "100.0");
    assert_eq!(val::render_value(&Value::Double(100.0)), "100");
    // `String.valueOf(null)` 是字符串 "null"（不是空串）
    assert_eq!(val::java_tostring(&Value::Null), "null");
}

// =====================================================================
// 网格渲染
// =====================================================================

fn src(row: i32, col: i32, extend: ExtendMode) -> SourceRef {
    SourceRef::new(row, col, extend)
}

#[test]
fn render_calculated_writes_the_top_level_document_row_major() {
    let mut g = CalcGrid::new(8, 9);
    // 故意逆序放入，验证输出按 (row, col) 行优先
    let mut c31 = CalcCell::bare(3, 1, Value::Int(200));
    c31.src = Some(src(3, 1, ExtendMode::Default16));
    let mut c21 = CalcCell::bare(2, 1, Value::Int(100));
    c21.src = Some(src(2, 1, ExtendMode::None17));
    g.cells.push(c31);
    g.cells.push(c21);
    // 空格（value=Null 且无 disp_value）不落盘（CalcJson.java:51-53）
    g.cells.push(CalcCell::bare(4, 1, Value::Null));

    assert_eq!(
        render_calculated(&g),
        "{\"calculated\":{\"rows\":8,\"cols\":9,\"cells\":[\
         {\"r\":2,\"c\":1,\"value\":100,\"type\":\"int\",\"source\":\
         {\"src_row\":2,\"src_col\":1,\"row_merge\":1,\"col_merge\":1,\"extend\":17,\"sub_cells\":1}},\
         {\"r\":3,\"c\":1,\"value\":200,\"type\":\"int\",\"source\":\
         {\"src_row\":3,\"src_col\":1,\"row_merge\":1,\"col_merge\":1,\"extend\":16,\"sub_cells\":1}}\
         ]}}"
    );
}

#[test]
fn disp_value_is_written_only_when_it_differs_from_string_value_of() {
    // (a) value=null 且 disp_value="null" ⇒ 判等成立 ⇒ **不写**（String.valueOf(null)=="null"）
    let mut a = CalcCell::bare(2, 1, Value::Null);
    a.disp_value = Some("null".to_string());
    assert_eq!(
        render_calculated(&CalcGrid { rows: 1, cols: 1, cells: vec![a] }),
        "{\"calculated\":{\"rows\":1,\"cols\":1,\"cells\":[\
         {\"r\":2,\"c\":1,\"value\":null,\"type\":\"null\"}]}}"
    );

    // (b) value=null 且 disp_value="仅显示值" ⇒ 写（engine_disp 的 (2,3) 形态）
    let mut b = CalcCell::bare(2, 3, Value::Null);
    b.disp_value = Some("仅显示值".to_string());
    assert_eq!(
        render_calculated(&CalcGrid { rows: 1, cols: 1, cells: vec![b] }),
        "{\"calculated\":{\"rows\":1,\"cols\":1,\"cells\":[\
         {\"r\":2,\"c\":3,\"value\":null,\"type\":\"null\",\
         \"disp_value\":\"\\u4ec5\\u663e\\u793a\\u503c\"}]}}"
    );

    // (c) Double(100.0) 与 disp_value="100.0" ⇒ 判等成立（用 Object.toString）⇒ 不写
    let mut c = CalcCell::bare(2, 4, Value::Double(100.0));
    c.disp_value = Some("100.0".to_string());
    assert_eq!(
        render_calculated(&CalcGrid { rows: 1, cols: 1, cells: vec![c] }),
        "{\"calculated\":{\"rows\":1,\"cols\":1,\"cells\":[\
         {\"r\":2,\"c\":4,\"value\":100,\"type\":\"double\"}]}}"
    );
}

#[test]
fn heads_and_sub_values_follow_the_optional_key_rules() {
    let mut c = CalcCell::bare(2, 1, Value::Text("R1".into()));
    c.left_head = Some((0, 0));
    c.top_head = Some((2, 2));
    c.src = Some(SourceRef {
        src_row: 2,
        src_col: 1,
        row_merge: 1,
        col_merge: 5,
        extend: ExtendMode::Vertical18,
        sub_cells: 2,
    });
    c.sub_values = vec![
        SubValue::new(2, 1, Value::Text("R1".into())),
        SubValue::new(3, 1, Value::Text("R2".into())),
    ];
    assert_eq!(
        render_calculated(&CalcGrid { rows: 8, cols: 9, cells: vec![c] }),
        "{\"calculated\":{\"rows\":8,\"cols\":9,\"cells\":[\
         {\"r\":2,\"c\":1,\"value\":\"R1\",\"type\":\"string\",\
         \"left_head\":{\"src_row\":0,\"src_col\":0},\"top_head\":{\"src_row\":2,\"src_col\":2},\
         \"source\":{\"src_row\":2,\"src_col\":1,\"row_merge\":1,\"col_merge\":5,\"extend\":18,\"sub_cells\":2,\
         \"sub_values\":[{\"r\":2,\"c\":1,\"value\":\"R1\",\"type\":\"string\"},\
         {\"r\":3,\"c\":1,\"value\":\"R2\",\"type\":\"string\"}]}}]}}"
    );
}

#[test]
fn render_calculated_error_writes_only_class_and_chain() {
    // chain 缺省 ⇒ 省略键（GenEngineFixtures.java:254-256 同策略）
    assert_eq!(
        render_calculated_error("com.raqsoft.commonl.ReportError", None),
        "{\"calculated\":{\"error\":{\"class\":\"com.raqsoft.commonl.ReportError\"}}}"
    );
    assert_eq!(
        render_calculated_error("com.raqsoft.commonl.ReportError", Some("B2-C2-B2-C2")),
        "{\"calculated\":{\"error\":{\"class\":\"com.raqsoft.commonl.ReportError\",\
         \"chain\":\"B2-C2-B2-C2\"}}}"
    );
}
