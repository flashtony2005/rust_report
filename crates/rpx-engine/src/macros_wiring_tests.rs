//! M7-8 宏的**接线**判据 —— 证明替换真的发生在**建树之前**，且真的驱动了引擎。
//!
//! # 为什么单独一份，不并进 `macros.rs` 的单测
//!
//! `macros.rs` 的单测证明 [`crate::macros::substitute`] 这个**纯函数**对；
//! 本文件证明它**接对了地方** —— 这是纯函数测不到的那一段（同一个函数接在
//! `parse` 之后、或只接在单元格表达式上、或压根没接，纯函数测都全绿）。
//!
//! # 判据的构造手法
//!
//! 拿一个真夹具，把它某个格的表达式原文**换成一个宏引用** `${m}`，宏表里
//! `m = 原文`。于是：
//!
//! * **接对了** ⇒ 展开回原文 ⇒ AST 与直接解析原文**逐节点相等**；
//! * **没接 / 接晚了** ⇒ 词法层拿到 `${m}`，报的是语法错（`UnsupportedExpr` 等），
//!   **不是** `UnknownMacro` —— 这正是两条断言的判别力所在。
//!
//! 反证（空宏表）单列一条：必须报 [`EngineError::UnknownMacro`]，且错误文本里带宏名。
//! 若实现退化成"静默留着 `${m}`"，这一条会红成语法错。

use std::path::PathBuf;

use rpx_core::reader::ObjValue;
use rpx_core::ReportDefine;

use crate::error::EngineError;
use crate::expr::Expr;
use crate::macros::MacroTable;
use crate::params::ParamTable;
use crate::stage_b::{cell_expression, prepare_calculate_with, StageB, CELL_EXPR_PROP};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn open(name: &str) -> ReportDefine {
    let path = fixtures_dir().join(format!("{name}.rpx"));
    let file =
        std::fs::File::open(&path).unwrap_or_else(|e| panic!("打开 {} 失败：{e}", path.display()));
    rpx_core::open(file).unwrap_or_else(|e| panic!("解析 {name}.rpx 失败：{e}"))
}

/// 找一个**本来就有表达式**的格（自检：找不到就 panic，不静默跳过）。
fn first_expression_cell(rd: &ReportDefine) -> (usize, usize, String) {
    (1..rd.rows)
        .flat_map(|r| (1..rd.cols).map(move |c| (r, c)))
        .find_map(|(r, c)| {
            let t = rd.cell_at(r, c)?;
            Some((r, c, cell_expression(t)?.to_string()))
        })
        .expect("engine_expr 里应至少有一个带表达式的格")
}

/// 把 `(row, col)` 格的表达式原文换成 `replacement`。
fn rewrite_cell_expression(rd: &mut ReportDefine, row: usize, col: usize, replacement: &str) {
    let id = rd.grid[row][col].unwrap_or_else(|| panic!("({row},{col}) 是空槽"));
    rd.store[id].exp_map = Some(vec![(
        CELL_EXPR_PROP,
        ObjValue::Str(replacement.to_string()),
    )]);
}

fn expr_of(sb: &StageB, row: i32, col: i32) -> Expr {
    let id = *sb
        .grid
        .by_pos
        .get(&(row, col))
        .unwrap_or_else(|| panic!("阶段 B 缺格 ({row},{col})"));
    let arc = sb.grid.cells[id.0 as usize]
        .rt
        .expr
        .clone()
        .unwrap_or_else(|| panic!("({row},{col}) 没有落盘表达式"));
    (*arc).clone()
}

/// 单元格表达式：`${m}` 必须在 `parse_with` **之前**被展开。
#[test]
fn a_macro_is_expanded_before_the_cell_expression_is_parsed() {
    let mut rd = open("engine_expr");
    let (row, col, original) = first_expression_cell(&rd);

    rewrite_cell_expression(&mut rd, row, col, "${m}");

    let mut macros = MacroTable::new();
    macros.insert("m", original.clone());
    let sb = prepare_calculate_with(&rd, &ParamTable::EMPTY, &macros)
        .unwrap_or_else(|e| panic!("带宏表跑阶段 B 失败：{e}"));

    // 展开回原文 ⇒ AST 必须与**直接解析原文**逐节点相等。
    assert_eq!(
        expr_of(&sb, row as i32, col as i32),
        crate::expr::parse(&original).unwrap(),
        "`${{m}}` 展开后必须与直接解析原文得到同一棵树（原文 = {original}）"
    );
}

/// `%{m}` 是设计文档的记号，走**同一条**接线（不是另一条代码路径）。
#[test]
fn the_design_doc_sigil_takes_the_same_wire() {
    let mut rd = open("engine_expr");
    let (row, col, original) = first_expression_cell(&rd);
    rewrite_cell_expression(&mut rd, row, col, "%{m}");

    let mut macros = MacroTable::new();
    macros.insert("m", original.clone());
    let sb = prepare_calculate_with(&rd, &ParamTable::EMPTY, &macros).unwrap();
    assert_eq!(
        expr_of(&sb, row as i32, col as i32),
        crate::expr::parse(&original).unwrap()
    );
}

/// 属性表达式（行属性 `expMap`）走的是**第二个**建树点，同样要吃宏。
#[test]
fn a_macro_is_expanded_before_a_property_expression_is_parsed() {
    let mut rd = open("engine_rowheight");
    // 行 2 的行高属性（key 1）—— 与 `stage_b_tests` 的 `(2, "40")` 同一格。
    let rc = &mut rd.row_cells[1];
    rc.exp_map = Some(vec![(1, ObjValue::Str("${h}".to_string()))]);

    let mut macros = MacroTable::new();
    macros.insert("h", "40");
    let sb = prepare_calculate_with(&rd, &ParamTable::EMPTY, &macros).unwrap();

    let id = *sb.grid.by_pos.get(&(2, 0)).expect("行 2 应有行头格");
    let got = &sb.grid.cells[id.0 as usize].rt.property_exprs;
    assert_eq!(
        got,
        &vec![(1u8, crate::expr::parse("40").unwrap())],
        "行属性的 `${{h}}` 必须展开成 40"
    );
}

/// 反证：宏表**非空、但没有这个名字** ⇒ 显式报 `UnknownMacro`（带宏名）。
///
/// ⚠ 为什么不用空表：空表是**另一条**路径 —— [`crate::macros::substitute`] 对空表
/// **不扫描**（"没登记任何宏"与"模板里有宏引用"是两件事），`${m}` 会原样落到词法层，
/// 报的是语法错。那条路径也**不静默**，但错误文本不同 ⇒ 拿空表测不出"未登记宏"。
/// 两条路径各有一条判据（下一条钉空表）。
#[test]
fn an_unregistered_name_is_an_explicit_error() {
    let mut rd = open("engine_expr");
    let (row, col, _) = first_expression_cell(&rd);
    rewrite_cell_expression(&mut rd, row, col, "${m}");

    let mut macros = MacroTable::new();
    macros.insert("other", "1"); // 表非空，但**没有** `m`
    let err = prepare_calculate_with(&rd, &ParamTable::EMPTY, &macros)
        .expect_err("宏表里没有 `m` 时必须显式失败");
    assert!(
        matches!(err, EngineError::UnknownMacro { ref name } if name == "m"),
        "应为 UnknownMacro{{m}}，实得：{err}"
    );
    assert!(err.to_string().contains('m'), "{err}");
}

/// 宏表为空时，宏引用也必须在建树前报告准确的 UnknownMacro。
#[test]
fn an_empty_macro_table_reports_the_missing_name_before_parsing() {
    let mut rd = open("engine_expr");
    let (row, col, _) = first_expression_cell(&rd);
    rewrite_cell_expression(&mut rd, row, col, "${m}");

    let err = prepare_calculate_with(&rd, &ParamTable::EMPTY, &MacroTable::EMPTY)
        .expect_err("`${m}` 在任何情况下都不该被静默接受");
    assert!(
        matches!(err, EngineError::UnknownMacro { .. }),
        "应在建树前报告未登记宏，实得：{err}"
    );
}
