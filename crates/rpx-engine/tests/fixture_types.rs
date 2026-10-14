//! `engine_types` 的类型面断言：把 M2-14（值类型保真）从"接口就位"推到
//! "产品夹具可断言"。
//!
//! # 为什么需要
//!
//! 7 个既有 `engine_*` 夹具只产出 `string`/`int`/`list` 三种类型
//! （`tests/fixtures/_shape_audit.py` 实测），而 `CalcJson.typeName`
//! （`CalcJson.java:224-253`）有 9 个分支 —— 即 6 个分支从未被真实数据走到。
//! 于是「`Integer(100)` 不能变成 `Double(100.0)`」这条 M2-14 的核心分叉
//! 在真实夹具上无法断言。`engine_types` 用纯字面量把
//! `double`/`long`/`decimal`/`boolean`/`float` 各钉至少一格。
//!
//! # 纪律
//!
//! 类型由 `type` 字段决定，**不由 JSON 字面形状决定**
//! （详见 `fixture_loader.rs` 里 `reader_preserves_exact_number_tokens_and_declared_type`
//! 的说明与 `CalcJson.java:204-212`）。

mod common;

use std::collections::BTreeMap;

use common::load;

/// 逐格真值：(r, c, declared_type, value_raw)。
///
/// 两个关键格：
/// * `(2,4)` 是 `Double(100.0)`，但 `CalcJson.value()` 把整数值的 double 折叠成
///   裸整数（`CalcJson.java:209-211`）⇒ `value_raw == "100"` 而
///   `declared_type == "double"`。
/// * `(4,1)` 是 `BigDecimal("100.00")`，`toString()` 原样（`:201-203`）
///   ⇒ `value_raw == "100.00"`，`declared_type == "decimal"`。
///
/// 只按字面形状猜类型的实现会恰好在这两格上翻车 —— 这正是本夹具的价值。
const CELLS: [(i32, i32, &str, &str); 16] = [
    (1, 1, "string", "\"int\""),
    (1, 2, "string", "\"long\""),
    (1, 3, "string", "\"double-frac\""),
    (1, 4, "string", "\"double-integral\""),
    (2, 1, "int", "100"),
    (2, 2, "long", "1234567890123"),
    (2, 3, "double", "92.5"),
    (2, 4, "double", "100"),
    (3, 1, "string", "\"decimal\""),
    (3, 2, "string", "\"boolean-true\""),
    (3, 3, "string", "\"boolean-false\""),
    (3, 4, "string", "\"float\""),
    (4, 1, "decimal", "100.00"),
    (4, 2, "boolean", "true"),
    (4, 3, "boolean", "false"),
    (4, 4, "float", "1.5"),
];

#[test]
fn engine_types_pins_every_cell_type_and_raw_token() {
    let fx = load("engine_types");
    assert_eq!((fx.template_rows, fx.template_cols), (4, 4), "模板尺寸");
    let g = fx.grid().expect("engine_types 有计算态网格");
    assert_eq!((g.rows, g.cols), (4, 4));

    for (r, c, ty, raw) in CELLS {
        let cell = g
            .cell(r, c)
            .unwrap_or_else(|| panic!("engine_types 缺 ({r},{c})"));
        assert_eq!(cell.declared_type, ty, "({r},{c}) declared_type");
        assert_eq!(cell.value_raw, raw, "({r},{c}) value_raw（原文 token）");
        assert!(cell.source.is_some(), "({r},{c}) 在模板内，必有 source");
    }
    assert_eq!(g.cells.len(), CELLS.len(), "非空格数应与真值表等长");
}

#[test]
fn engine_types_histogram_covers_seven_types() {
    let fx = load("engine_types");
    let g = fx.grid().unwrap();
    let mut hist: BTreeMap<&str, i32> = BTreeMap::new();
    for cell in &g.cells {
        *hist.entry(cell.declared_type.as_str()).or_insert(0) += 1;
    }
    // 7 种类型（既有 3 种 + 新增 4 种）；8 个 string 都是标签格
    assert_eq!(
        hist,
        BTreeMap::from([
            ("string", 8),
            ("int", 1),
            ("long", 1),
            ("double", 2),
            ("decimal", 1),
            ("boolean", 2),
            ("float", 1),
        ])
    );
}

/// M2-14 的核心分叉单独钉一次：`Integer(100)` 与 `Double(100.0)` 的 `value_raw`
/// 形状相同（都是 `100`），**只有 `declared_type` 分得开**。
#[test]
fn integer_and_integral_double_differ_only_by_declared_type() {
    let fx = load("engine_types");
    let int_cell = fx.cell(2, 1).unwrap();
    let dbl_cell = fx.cell(2, 4).unwrap();

    assert_eq!(
        int_cell.value_raw, dbl_cell.value_raw,
        "两者字面形状都是 100"
    );
    assert_eq!(int_cell.declared_type, "int");
    assert_eq!(dbl_cell.declared_type, "double");
    assert_ne!(
        int_cell.declared_type, dbl_cell.declared_type,
        "类型必须靠 type 字段区分，不能靠 value 字面"
    );
}
