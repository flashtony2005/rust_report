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
use super::{
    render_calculated, render_calculated_error, render_document, render_merged, render_pagination,
};
use crate::arena::{CalcCell, CalcGrid, MergedCell, SourceRef, SubValue};
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
    assert_eq!(
        val::render_value(&Value::Text("N\"orth".into())),
        "\"N\\\"orth\""
    );
    assert_eq!(val::render_value(&Value::Bool(true)), "true");
    assert_eq!(val::render_value(&Value::Int(100)), "100");
    assert_eq!(
        val::render_value(&Value::Long(1234567890123)),
        "1234567890123"
    );
    // BigDecimal 原样搬（`100.0` 的尾零是精度信息，不许重排成 `100`）
    assert_eq!(
        val::render_value(&Value::Decimal("100.00".into())),
        "100.00"
    );
    // 整数值的 Double 被折叠成裸整数（CalcJson.java:241-243）
    assert_eq!(val::render_value(&Value::Double(100.0)), "100");
    assert_eq!(val::render_value(&Value::Double(92.5)), "92.5");
    assert_eq!(val::render_value(&Value::Float(1.5)), "1.5");
    // NaN / Inf 走**带引号**字符串，且用的是 double 的写法
    assert_eq!(val::render_value(&Value::Double(f64::NAN)), "\"NaN\"");
    assert_eq!(
        val::render_value(&Value::Float(f32::INFINITY)),
        "\"Infinity\""
    );
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
        render_calculated(&CalcGrid {
            rows: 1,
            cols: 1,
            cells: vec![a],
            ..CalcGrid::new(1, 1)
        }),
        "{\"calculated\":{\"rows\":1,\"cols\":1,\"cells\":[\
         {\"r\":2,\"c\":1,\"value\":null,\"type\":\"null\"}]}}"
    );

    // (b) value=null 且 disp_value="仅显示值" ⇒ 写（engine_disp 的 (2,3) 形态）
    let mut b = CalcCell::bare(2, 3, Value::Null);
    b.disp_value = Some("仅显示值".to_string());
    assert_eq!(
        render_calculated(&CalcGrid {
            rows: 1,
            cols: 1,
            cells: vec![b],
            ..CalcGrid::new(1, 1)
        }),
        "{\"calculated\":{\"rows\":1,\"cols\":1,\"cells\":[\
         {\"r\":2,\"c\":3,\"value\":null,\"type\":\"null\",\
         \"disp_value\":\"\\u4ec5\\u663e\\u793a\\u503c\"}]}}"
    );

    // (c) Double(100.0) 与 disp_value="100.0" ⇒ 判等成立（用 Object.toString）⇒ 不写
    let mut c = CalcCell::bare(2, 4, Value::Double(100.0));
    c.disp_value = Some("100.0".to_string());
    assert_eq!(
        render_calculated(&CalcGrid {
            rows: 1,
            cols: 1,
            cells: vec![c],
            ..CalcGrid::new(1, 1)
        }),
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
        render_calculated(&CalcGrid {
            rows: 8,
            cols: 9,
            cells: vec![c],
            ..CalcGrid::new(8, 9)
        }),
        "{\"calculated\":{\"rows\":8,\"cols\":9,\"cells\":[\
         {\"r\":2,\"c\":1,\"value\":\"R1\",\"type\":\"string\",\
         \"left_head\":{\"src_row\":0,\"src_col\":0},\"top_head\":{\"src_row\":2,\"src_col\":2},\
         \"source\":{\"src_row\":2,\"src_col\":1,\"row_merge\":1,\"col_merge\":5,\"extend\":18,\"sub_cells\":2,\
         \"sub_values\":[{\"r\":2,\"c\":1,\"value\":\"R1\",\"type\":\"string\"},\
         {\"r\":3,\"c\":1,\"value\":\"R2\",\"type\":\"string\"}]}}]}}"
    );
}

/// `merged` 段的键序（`CalcMergeJson.java:51-57`：`page, r, c, value, row_merge, col_merge`）
/// 与**「不跳空值格」**这条与 `cells` 相反的规则（`:48-50`）。
///
/// **为什么不是空跑**：第一项故意用 `Value::Null` 且**没有** `disp_value` —— 这种格在
/// `calculated.cells` 里会被稀疏规则剔掉（上面 `render_calculated_writes_…` 已钉），
/// 在 `merged.cells` 里却必须留下。若谁把两段的稀疏规则写成一样，本测试立刻红。
#[test]
fn render_merged_keeps_null_valued_cells_and_uses_the_fixture_key_order() {
    let mut g = CalcGrid::new(8, 4);
    g.page_count = 1;
    // 故意逆序放入，验证输出按 (page, r, c) 排序
    g.merged
        .push(MergedCell::new(1, 3, 1, Value::Text("North".into()), 2, 1));
    g.merged.push(MergedCell::new(1, 2, 3, Value::Null, 1, 2));
    // 跨度全 1 的格**不进** merged（`CalcMergeJson.java:48-50` 的判据在导出侧）
    assert_eq!(
        render_merged(&g),
        "{\"merged\":{\"page_count\":1,\"cells\":[\
         {\"page\":1,\"r\":2,\"c\":3,\"value\":null,\"row_merge\":1,\"col_merge\":2},\
         {\"page\":1,\"r\":3,\"c\":1,\"value\":\"North\",\"row_merge\":2,\"col_merge\":1}]}}"
    );
}

/// M5-02：`merged` 段的**页内序列化**（`CalcMergeJson.renderMerged` 读页格）。
///
/// 形态复刻 `cross_emp_bonus` 真值（引擎产出的 `merged_pages`）：标题 1×9 合并
/// 被横向断页拆到两页（第 1 页 8 列 / 第 2 页 1 列）+ 一块 2×1 纵向合并。
/// 投影本身（合并区 × 页矩形求交）在 `calc/merged_view.rs` 单测；本条钉子
/// emit 的序列化契约：`page_count` 取网格字段、逐条 `(page, r, c)` 排序、
/// 键序 `page, r, c, value, row_merge, col_merge`。
///
/// **为什么不是空跑**：第 2 页标题槽交矩形 `1×1` 被投影跳过（`16 条不是 17 条`，
/// 见 `merged_view` 单测），所以本条的输入里**没有**第 2 页条目 —— 若谁把
/// 全局视图（`merged`，13 条、坐标是全局）直接序列化出来，本条立刻红。
#[test]
fn render_merged_serializes_the_page_local_view() {
    let mut g = CalcGrid::new(9, 9);
    g.page_count = 2;
    // 引擎分页后挂上的页内视图（merged_view::page_slots 的产出形态）。
    for c in 1..=8 {
        g.merged_pages
            .push(MergedCell::new(1, 1, c, Value::Text("T".into()), 1, 8));
    }
    for r in 8..=9 {
        g.merged_pages
            .push(MergedCell::new(1, r, 1, Value::Int(1), 2, 1));
        g.merged_pages
            .push(MergedCell::new(1, r, 2, Value::Text("N".into()), 2, 1));
    }
    // 全局视图同时在（服务面快照吃它）—— 不得被序列化进 merged 段。
    for c in 1..=9 {
        g.merged
            .push(MergedCell::new(1, 1, c, Value::Text("T".into()), 1, 9));
    }
    for r in 8..=9 {
        g.merged.push(MergedCell::new(1, r, 1, Value::Int(1), 2, 1));
        g.merged
            .push(MergedCell::new(1, r, 2, Value::Text("N".into()), 2, 1));
    }
    g.pagination = Some(crate::pager::Pagination {
        paper_w: 595.0,
        paper_h: 842.0,
        columns: 1,
        page_count: 2,
        x_pages: 2,
        y_pages: 1,
        row_num_per_page: 20,
        pages: vec![
            crate::pager::PageInfo {
                panels: Vec::new(),
                page: 1,
                rows: 9,
                col1_values: Vec::new(),
                row_indices: (1..=9).collect(),
                row_range: (1, 9),
                col_indices: (1..=8).collect(),
                col_range: (1, 8),
            },
            crate::pager::PageInfo {
                panels: Vec::new(),
                page: 2,
                rows: 9,
                col1_values: Vec::new(),
                row_indices: (1..=9).collect(),
                row_range: (1, 9),
                col_indices: (9..=9).collect(),
                col_range: (9, 9),
            },
        ],
    });
    assert_eq!(
        render_merged(&g),
        "{\"merged\":{\"page_count\":2,\"cells\":[\
         {\"page\":1,\"r\":1,\"c\":1,\"value\":\"T\",\"row_merge\":1,\"col_merge\":8},\
         {\"page\":1,\"r\":1,\"c\":2,\"value\":\"T\",\"row_merge\":1,\"col_merge\":8},\
         {\"page\":1,\"r\":1,\"c\":3,\"value\":\"T\",\"row_merge\":1,\"col_merge\":8},\
         {\"page\":1,\"r\":1,\"c\":4,\"value\":\"T\",\"row_merge\":1,\"col_merge\":8},\
         {\"page\":1,\"r\":1,\"c\":5,\"value\":\"T\",\"row_merge\":1,\"col_merge\":8},\
         {\"page\":1,\"r\":1,\"c\":6,\"value\":\"T\",\"row_merge\":1,\"col_merge\":8},\
         {\"page\":1,\"r\":1,\"c\":7,\"value\":\"T\",\"row_merge\":1,\"col_merge\":8},\
         {\"page\":1,\"r\":1,\"c\":8,\"value\":\"T\",\"row_merge\":1,\"col_merge\":8},\
         {\"page\":1,\"r\":8,\"c\":1,\"value\":1,\"row_merge\":2,\"col_merge\":1},\
         {\"page\":1,\"r\":8,\"c\":2,\"value\":\"N\",\"row_merge\":2,\"col_merge\":1},\
         {\"page\":1,\"r\":9,\"c\":1,\"value\":1,\"row_merge\":2,\"col_merge\":1},\
         {\"page\":1,\"r\":9,\"c\":2,\"value\":\"N\",\"row_merge\":2,\"col_merge\":1}]}}"
    );
    // 摘掉 pagination ⇒ 回落全局视图（13 条整段跨度）—— 服务面快照的形态。
    g.pagination = None;
    let doc = render_merged(&g);
    assert_eq!(doc.matches("\"page\":1").count(), 13, "全局视图 13 槽全在");
    assert!(
        doc.contains("\"r\":1,\"c\":9,\"value\":\"T\",\"row_merge\":1,\"col_merge\":9"),
        "全局视图中标题第 9 槽必须是整段跨度 9（页内投影才裁成 8）"
    );
}

/// `render_document` = `calculated` + `pagination` + `merged` 三段（夹具顶层键序：
/// `…, calculated, pagination, merged`）。
///
/// **为什么不是空跑**：除了钉住字面量，还断言「顶层文档 == 三个单段文档去壳拼接」——
/// 这条身份式能同时抓住「漏掉一段」「拼接处多/少逗号」与「三段用了不同的稀疏规则」，
/// 而只比字面量抓不到最后一条。
#[test]
fn render_document_concatenates_all_three_sections() {
    let mut g = CalcGrid::new(1, 1);
    g.cells.push(CalcCell::bare(1, 1, Value::Int(7)));
    g.merged.push(MergedCell::new(1, 1, 1, Value::Int(7), 1, 3));
    let doc = render_document(&g);
    assert_eq!(
        doc,
        "{\"calculated\":{\"rows\":1,\"cols\":1,\"cells\":[\
         {\"r\":1,\"c\":1,\"value\":7,\"type\":\"int\"}]},\
         \"pagination\":null,\
         \"merged\":{\"page_count\":1,\"cells\":[\
         {\"page\":1,\"r\":1,\"c\":1,\"value\":7,\"row_merge\":1,\"col_merge\":3}]}}"
    );
    // 身份式：三个单段文档各自去掉最外层大括号后拼接
    let parts = [
        render_calculated(&g),
        render_pagination(&g),
        render_merged(&g),
    ];
    let joined: Vec<&str> = parts.iter().map(|s| &s[1..s.len() - 1]).collect();
    assert_eq!(
        doc,
        format!("{{{}}}", joined.join(",")),
        "顶层文档必须由三个单段渲染器拼成，不得另走一套规则"
    );
}

/// 有分页结果时 `pagination` 段的**逐字符**形状（键序 + float 记法 + 可选键）。
///
/// ⛔ 三条判据各有判别力：
/// * `paper_w` 写 **`595.0`** 而不是 `595` —— 它是 `JsonObj.put(String, float)`；
/// * `first_value` / `last_value` 是 `String.valueOf`（走 `java_tostring`），
///   而 `col1_values` 是 `value()` 的原文 token：同一页里 `Double(100.0)` 在前者写
///   `"100.0"`、在后者写 `100`。这里用 `Double(100.0)` 当首值正是为了钉住这条分家；
/// * 整列非空 ⇒ 两个可选键**都**出现；整列全空 ⇒ 两个键**都不**出现（夹具的
///   `engine_pager` 第 4 页就是"rows 4 但只有 3 个非空值"的形态）。
#[test]
fn render_pagination_pins_key_order_float_notation_and_optional_keys() {
    let mut g = CalcGrid::new(2, 1);
    g.pagination = Some(crate::pager::Pagination {
        paper_w: 595.0,
        paper_h: 842.0,
        columns: 1,
        page_count: 2,
        x_pages: 1,
        y_pages: 2,
        row_num_per_page: 20,
        pages: vec![
            crate::pager::PageInfo {
                panels: Vec::new(),
                page: 1,
                rows: 2,
                col1_values: vec![Value::Double(100.0), Value::Text("尾".into())],
                // M3-06 新增字段：emit 逐键序列化不读它们 ⇒ 输出不变（下面断言钉死）。
                row_indices: (1..=2).collect(),
                row_range: (1, 2),
                col_indices: (1..=1).collect(),
                col_range: (1, 1),
            },
            crate::pager::PageInfo {
                panels: Vec::new(),
                page: 2,
                rows: 1,
                col1_values: Vec::new(),
                row_indices: (2..=2).collect(),
                row_range: (2, 2),
                col_indices: (1..=1).collect(),
                col_range: (1, 1),
            },
        ],
    });
    assert_eq!(
        render_pagination(&g),
        "{\"pagination\":{\"paper_w\":595.0,\"paper_h\":842.0,\"columns\":1,\
         \"page_count\":2,\"x_pages\":1,\"y_pages\":2,\"row_num_per_page\":20,\
         \"pages\":[\
         {\"page\":1,\"rows\":2,\"first_value\":\"100.0\",\"last_value\":\"\\u5c3e\",\
         \"col1_values\":[100,\"\\u5c3e\"]},\
         {\"page\":2,\"rows\":1,\"col1_values\":[]}]}}"
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
