//! `cell_role_of` 与真实 `expr` 模块的接线测试（§12.1 第 3 步验收）。
//!
//! 这一组钉住的是**接线本身**：此前 `cell_role` 的 `expr_is_extend` 形参没有任何真实
//! 调用方 —— 判定链是断的，实现错也没人撞得到。分文件是因为 `extend/tests.rs`
//! 已顶破 300 行硬门禁。

use super::super::role::{cell_role, cell_role_of, CellRole};
use super::super::{ExtendMode, EXTEND_EXCLUDED_CELL_TYPES};

/// 普通格 cellType（不是被排除的三种之一）。
const NORMAL_TYPE: i8 = -64;
/// 条形码（被排除的类型之一，`ExtNormalCell.java:494-496`）。
const BARCODE_TYPE: i8 = -57;

#[test]
fn cell_role_of_resolves_a_select_into_an_extender() {
    // 声明 16 + 驱动扩展的表达式 ⇒ Extender（随后按拓扑改写成 18/19）
    assert_eq!(
        cell_role_of(
            Some("ds1.select(region)"),
            ExtendMode::Default16,
            NORMAL_TYPE
        )
        .unwrap(),
        CellRole::Extender
    );
}

#[test]
fn cell_role_of_a_plain_arithmetic_formula_is_not_an_extender() {
    // 声明 16 + 不驱动扩展的表达式 ⇒ PlainFormula（随后无条件置 17）
    assert_eq!(
        cell_role_of(Some("B2*2"), ExtendMode::Default16, NORMAL_TYPE).unwrap(),
        CellRole::PlainFormula
    );
}

#[test]
fn cell_role_of_no_expression_keeps_the_declared_literal() {
    // 无表达式 + 声明 16 ⇒ Literal，声明值原样保留（**不改写**）
    assert_eq!(
        cell_role_of(None, ExtendMode::Default16, NORMAL_TYPE).unwrap(),
        CellRole::Literal
    );
}

#[test]
fn cell_role_of_declared_18_wins_even_without_an_expression() {
    // ⚠ 判定**顺序**是契约，不是"有没有表达式"优先（`ExtNormalCell.java:492-514`）：
    //   ① 类型短路 → ② `return 18 == extend || 19 == extend` → ③ 才看有没有表达式。
    // 所以声明 18/19 时**即便没有表达式**也是 Extender（走 ExtExtendCell 路径，
    // 由运行时 `result instanceof List` 决定是否真扩展 —— 见附件 I.1b）。
    // 我第一版把它断言成 Literal，被这条顺序纠正了 —— 顺序写错会静默改变扩展行数。
    assert_eq!(
        cell_role_of(None, ExtendMode::Vertical18, NORMAL_TYPE).unwrap(),
        CellRole::Extender
    );
    assert_eq!(
        cell_role_of(None, ExtendMode::Horizontal19, NORMAL_TYPE).unwrap(),
        CellRole::Extender
    );
}

#[test]
fn cell_role_of_excluded_cell_type_never_becomes_an_extender() {
    // 类型短路优先于一切 extend 判断。即便表达式驱动扩展、即便声明 18，
    // 图形/子报表/条形码也退回普通格规则（有表达式就 17）。
    for t in EXTEND_EXCLUDED_CELL_TYPES {
        assert_eq!(
            cell_role_of(Some("ds1.select(region)"), ExtendMode::Vertical18, t).unwrap(),
            CellRole::PlainFormula,
            "cellType={t} 必须被短路排除"
        );
    }
}

#[test]
fn cell_role_of_propagates_a_parse_error_instead_of_silently_false() {
    // ⛔ 静默假设 false 会把本该扩展的格判成 PlainFormula ⇒ 扩展行数算少，
    // 且报错点离根因极远。解析失败必须返回错误。
    let bad = cell_role_of(Some("ds1.select("), ExtendMode::Default16, NORMAL_TYPE);
    assert!(bad.is_err(), "残缺表达式必须报错，实际 {bad:?}");
}

#[test]
fn cell_role_of_agrees_with_the_manual_form() {
    // 接线路径与手工传参路径必须给出同一结论 —— 否则"接线"本身就引入了分叉。
    let cases: [(Option<&str>, ExtendMode, i8); 4] = [
        (
            Some("ds1.select(region)"),
            ExtendMode::Default16,
            NORMAL_TYPE,
        ),
        (Some("B2*2"), ExtendMode::Default16, NORMAL_TYPE),
        (None, ExtendMode::Horizontal19, NORMAL_TYPE),
        (
            Some("sort(ds1.select(x))"),
            ExtendMode::Default16,
            BARCODE_TYPE,
        ),
    ];
    for (exp, declared, ctype) in cases {
        let manual = match exp {
            None => cell_role(false, declared, false, ctype),
            Some(src) => {
                let e = crate::expr::is_extend(src).expect("表达式应可解析");
                cell_role(true, declared, e, ctype)
            }
        };
        assert_eq!(
            cell_role_of(exp, declared, ctype).unwrap(),
            manual,
            "exp={exp:?} declared={declared:?} ctype={ctype}"
        );
    }
}
