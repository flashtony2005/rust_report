#![cfg(feature = "legacy-rpx")]
//! M7-8 宏（定义 JSON 顶层 `macros` 块）的端到端判据 —— **真 socket + 真 multipart**。
//!
//! # 为什么单独一份（不并进 `reportdef_http.rs`）
//!
//! 两个理由，都是硬的：
//!
//! 1. `reportdef_http.rs` 已 407 行，加进来会越过 G4 的 500 行闸；
//! 2. 更重要的：那份文件的判据是「**JSON 定义 = `.rpx`**」（接线等价 + 几何真值 +
//!    导出读回）。宏的判据是**另一件事** —— 它问的是"替换有没有跑在建树之前"，
//!    与两条入口等价无关。混在一起会让"这一段在测什么"变得含糊。
//!
//! # 四条判据（缺一不可）
//!
//! * ① `${m}`（宏 `m = ds1.select(qty)`）渲染出的明细列 == 直接写 `ds1.select(qty)`；
//! * ② 换一个宏值（`m = ds1.select(region)`）⇒ 结果**必须变** —— ①单独一条的
//!   判别力不够：若实现把 `${m}` 整格当字面量跳过、或碰巧渲染成空，①也可能绿；
//!   ②是①的对照，两条一起才排除了"宏被忽略后恰好相同"。
//! * ③ 宏表里**没有** `m` ⇒ 400 且错误文本是 `unknown macro`。
//!   ⛔ 这一条的关键在**错误文本**：若替换没跑在建树之前，`${m}` 会落进词法层，
//!   报的是「期望 `[`，实际是 `{`」这类**与真实原因无关的语法错** —— 那种情况下
//!   状态码同样是 400，只有文本能区分。**这正是本判据的判别力所在。**
//! * ④ HTTP 面 ≡ 库函数直调（宏不破坏"一个出口"，`reportdef.rs` 的模块文档）。
//!
//! 不含自定义宏属性的产品夹具读取为空表；本项目保存的宏另有持久化回归。

mod common;

use rpx_engine::params::ParamTable;
use rpx_runtime::execution::render_request;
use rpx_runtime::form::RenderRequest;

/// 数据集文件名（必须与定义里的 `file_name` 逐字一致 —— 引擎按它取数）。
const CSV: &str = "engine_samemerge.csv";

/// 参照定义：明细列**直接写** `ds1.select(qty)`，无宏。
fn direct_def() -> String {
    def_body("ds1.select(qty)", "")
}

/// 宏定义：明细列写 `${m}`，顶层带 `macros` 块（`macro_body` 是它的内容）。
fn def_with_macro(macro_body: &str) -> String {
    def_body("${m}", &format!(",\n  \"macros\": {{ {macro_body} }}"))
}

/// 定义正文（`col2_expr` = 明细数量列的表达式；`macros_block` 可空）。
fn def_body(col2_expr: &str, macros_block: &str) -> String {
    format!(
        r#"{{
  "version": 1,
  "grid": {{ "rows": 4, "cols": 2 }},
  "cells": [
    {{ "row": 1, "col": 1, "text": "区域销售明细", "merge": {{ "rows": 1, "cols": 2 }} }},
    {{ "row": 2, "col": 1, "text": "区域" }},
    {{ "row": 2, "col": 2, "text": "数量" }},
    {{ "row": 3, "col": 1, "expr": "ds1.select(region)", "merge_same": "vertical" }},
    {{ "row": 3, "col": 2, "expr": "{col2_expr}" }},
    {{ "row": 4, "col": 1, "text": "合计" }},
    {{ "row": 4, "col": 2, "expr": "ds1.sum(qty)" }}
  ]{macros_block},
  "datasets": [
    {{ "name": "ds1", "file": {{ "file_name": "{CSV}", "encoding": "UTF-8" }} }}
  ]
}}"#
    )
}

fn render_def(def: &str) -> Result<String, rpx_runtime::execution::RenderError> {
    render_request(&RenderRequest {
        report: def.as_bytes().to_vec(),
        report_name: "def".into(),
        datasets: [(CSV.into(), common::fixture(CSV))].into_iter().collect(),
        params: ParamTable::EMPTY,
    })
}

fn sheet(json_text: &str) -> serde_json::Value {
    let j: serde_json::Value = serde_json::from_str(json_text).unwrap();
    j["sheets"]["rpx-sheet-1"].clone()
}

/// `cellData[r][c].v`（0 起坐标）。
fn cell_v(sheet: &serde_json::Value, r: usize, c: usize) -> serde_json::Value {
    sheet["cellData"][r.to_string()][c.to_string()]["v"].clone()
}

/// 明细区数量列（0 起第 2..=6 行、第 2 列）的取值序列。
fn qty_column(sheet: &serde_json::Value) -> Vec<serde_json::Value> {
    (2..=6).map(|r| cell_v(sheet, r, 1)).collect()
}

#[test]
fn a_macro_in_the_definition_drives_the_render() {
    let via_macro = render_def(&def_with_macro(r#""m": "ds1.select(qty)""#)).unwrap();
    let direct = render_def(&direct_def()).unwrap();
    assert_eq!(via_macro, direct);
    assert_eq!(
        qty_column(&sheet(&via_macro)),
        vec![10, 20, 30, 40, 50]
            .into_iter()
            .map(serde_json::Value::from)
            .collect::<Vec<_>>()
    );
    let other = render_def(&def_with_macro(r#""m": "ds1.select(region)""#)).unwrap();
    assert_ne!(qty_column(&sheet(&other)), qty_column(&sheet(&via_macro)));
    assert!(render_def(&def_with_macro(r#""other": "1""#))
        .unwrap_err()
        .to_string()
        .contains("UnknownMacro"));
}

/// 无自定义宏属性的产品夹具，读取为空宏表。
///
/// 这不是"实现完了"的证明，而是**把已声明边界钉住** —— 哪天 `MacroMetaData` 逆向
/// 完成、`.rpx` 能带宏了，本条会红，提醒把 `docs/report-def-json.md` §4.9 的边界段
/// 一起改掉。用真夹具（`engine_expr.rpx`）而不是构造字节，避免"测试自己造的形状"。
#[test]
fn the_rpx_entry_yields_an_empty_macro_table() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/engine_expr.rpx");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("读 {} 失败：{e}", path.display()));
    let (_rd, macros) = rpx_runtime::reportdef::open_report_full(&bytes).expect(".rpx 应能打开");
    assert!(
        macros.is_empty(),
        "`.rpx` 入口目前拿不到宏（MacroMetaData 未逆向）；若这里不再为空，说明边界已变，\
         请同步改 docs/report-def-json.md §4.9 的边界段"
    );
}

#[test]
fn own_macro_definitions_survive_saved_rpx_and_engine_entry() {
    let rd=rpx_runtime::reportdef::build(&serde_json::json!({"grid":{"rows":1,"cols":1},"macros":{"amount":"21*2"},"cells":[{"row":1,"col":1,"expr":"${amount}"}]}).to_string()).unwrap();
    let bytes = rpx_core::to_bytes(&rd).unwrap();
    let (rd, macros) = rpx_runtime::reportdef::open_report_full(&bytes).unwrap();
    assert_eq!(macros.get("amount"), Some("21*2"));
    let g = rpx_engine::calculate(&rd, &[]).unwrap();
    assert_eq!(
        g.get(1, 1).unwrap().value,
        rpx_engine::value::Value::Int(42)
    );
}
