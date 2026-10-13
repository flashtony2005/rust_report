//! `stage_b_tests/json.rs` 扫描器的自测。
//!
//! 拆成子模块的理由只有一条：**门禁是每个 `.rs` ≤ 300 行**，而扫描器本体
//! （词法 + 切片 + `Value` 渲染）已占满，测试再放进去就超了。
//!
//! 这里钉的三个风险都有共同的成因：**唯一断言 `value` 的夹具 `engine_types` 形状太干净
//! （无 `sub_values` / 无非 ASCII 显示值 / 无数字串）**，于是扫描器的退化变异在夹具上零信号。
//! 所以样例改用**内联 JSON 文本**，不依赖夹具。

use super::*;

/// `CalcJson.cellJson` 的键序是 `r, c, value, type, [disp_value], left_head, top_head, source`，
/// 而 `source.sub_values[]` 里**也有** `value`/`type` 两个同名键（`CalcJson.java:110-111`）。
///
/// ⛔ 这条测试钉的正是"取到的是**格自己的**、不是 `sub_values` 里那个同名键"：
/// 把 `raw_token_after` 的 `find` 换成 `rfind`（模拟误取最后一处）时必须失败。
/// 之所以需要它：唯一断言 `value` 的夹具 `engine_types` **没有** `sub_values`，
/// 而带 `sub_values` 的夹具目前没有任何测试断言其 `value` —— 即该风险零信号。
const CELL_WITH_SUBS: &str = concat!(
    r#"{"r":1,"c":1,"value":"outer","type":"string","left_head":{"src_row":0,"src_col":0},"#,
    r#""source":{"src_row":1,"src_col":1,"row_merge":1,"col_merge":1,"extend":16,"sub_cells":2,"#,
    r#""sub_values":[{"r":1,"c":1,"value":"innerA","type":"int"},{"r":1,"c":2,"value":"innerB","type":"double"}]}}"#
);

/// 带 `disp_value` 的格（`CalcJson` 只在显示值 ≠ `String.valueOf(value)` 时写这个键）。
const CELL_WITH_DISP: &str =
    r#"{"r":2,"c":3,"value":null,"type":"null","disp_value":"\u4ec5\u663e\u793a\u503c","left_head":{"src_row":0,"src_col":0}}"#;

fn parse_one(chunk: &str) -> ExpCell {
    let parts = split_top_level(chunk);
    assert_eq!(parts.len(), 1, "样例应被切成恰好 1 个顶层元素");
    parse_cell(parts[0])
}

#[test]
fn own_value_and_type_win_over_the_same_keys_inside_sub_values() {
    let e = parse_one(CELL_WITH_SUBS);
    assert_eq!(e.value.as_deref(), Some("\"outer\""), "取到了 sub_values 里的 value");
    assert_eq!(e.ty.as_deref(), Some("string"), "取到了 sub_values 里的 type");
    assert_eq!(e.subs, vec![(1, 1), (1, 2)]);
}

#[test]
fn disp_value_is_read_as_a_string_and_unicode_is_unescaped() {
    let e = parse_one(CELL_WITH_DISP);
    assert_eq!(e.value.as_deref(), Some("null"));
    assert_eq!(e.ty.as_deref(), Some("null"));
    // `\uXXXX` 必须解成真字符，否则与 Rust 侧的 `Value::Text` 永远比不相等
    assert_eq!(e.disp.as_deref(), Some("仅显示值"));
    assert!(e.subs.is_empty());
}

#[test]
fn quoted_strings_keep_their_quotes_so_strings_and_numbers_stay_distinguishable() {
    let e = parse_one(r#"{"r":1,"c":1,"value":"1000","type":"string"}"#);
    assert_eq!(e.value.as_deref(), Some("\"1000\""));
    let n = parse_one(r#"{"r":1,"c":2,"value":1000,"type":"int"}"#);
    assert_eq!(n.value.as_deref(), Some("1000"));
}
