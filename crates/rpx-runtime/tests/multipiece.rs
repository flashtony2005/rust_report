//! 主格 / 附属格 的扩展（教程 `dpkz101`《多片扩展》例 10.6.1 的最小化）。
//!
//! # 教程原文（`dpkz101`）
//!
//! > 单元格纵向扩展时，**左边纵向扩展单元格缺省为它的左主格，右边单元格缺省为它的附属格**；
//! > 如果左边没有纵向扩展格，则左主格缺省为 `` `0 `` 格。
//! > 人为改变主格规则：…… 可以将某个单元格的左主格设置成某个纵向扩展格……
//! > **人为设置时，可能发生左（上）主格在右（下）边的情况，而且主格也不一定和附属格
//! > 在同一行（列）上。**
//!
//! 例 10.6.1 的做法是：主表格（`B2 = ds1.select(订单ID)`）纵向扩展，然后**人为**把
//! `A1..A9` 一整片格的左主格都设成 `B2` ⇒ 那片格随主格一起复制。这就是「多片扩展」。
//!
//! # 判据（四组，互不重叠；期望值全部**手写**，不复用生产代码的中间产物）
//!
//! | 组 | 命题 | 观测面 |
//! |----|------|--------|
//! | 1 | 声明的附属格 ⇒ **随主格复制**（多片扩展） | `cellData` 逐格 + `mergeData` **为空** |
//! | 2 | 不声明且同级 ⇒ **被拉大**（合并），**不复制** | `mergeData` 恰一条 5 行矩形 |
//! | 3 | 缺省「右（下）格是附属格」 | `cellData` 逐格 |
//! | 4 | 主格在**右（下）边**照样成立（教程明写） | `cellData` 逐格 |
//!
//! 「复制 vs 拉大」是本文件的核心分野：两者在 `cellData` 上**长得很像**
//! （5 行都有值），只有 `mergeData` 能把它们分开 —— 所以第 1 组的
//! `mergeData` 必须**断言为空**，否则这条判据对「拉大」也是绿的（假绿）。
//!
//! 第 1 组与第 2 组是**同一份模板**（只差 `left_head` 两个字段），两条合起来才证明
//! 「声明」真的改变了扩展形态，而不是恰好长得一样。
//!
//! # 数据
//!
//! `engine_samemerge.csv`：`region` 列 = `A A B B C`（5 行，含相邻重复值）。

mod common;

use rpx_engine::params::ParamTable;
use rpx_runtime::execution::render_request;
use rpx_runtime::form::RenderRequest;

const CSV: &str = "engine_samemerge.csv";

/// 数据集里的 5 个 `region` 值（手写，与 CSV 字面量对齐）。
const REGIONS: [&str; 5] = ["A", "A", "B", "B", "C"];

// ---------------------------------------------------------------- 三个模板

/// ① 多片扩展：`左甲`(1,1) 与 `左乙`(2,1) 的左主格**人为**指向主格 `B1`。
fn def_declared() -> String {
    format!(
        r#"{{
  "version": 1,
  "grid": {{ "rows": 3, "cols": 3 }},
  "cells": [
    {{ "row": 1, "col": 1, "text": "左甲", "left_head": "B1" }},
    {{ "row": 2, "col": 1, "text": "左乙", "left_head": "B1" }},
    {{ "row": 1, "col": 2, "expr": "ds1.select(region)", "extend_mode": "vertical" }},
    {{ "row": 1, "col": 3, "text": "右" }},
    {{ "row": 3, "col": 1, "text": "尾" }}
  ],
  "datasets": [
    {{ "name": "ds1", "file": {{ "file_name": "{CSV}", "encoding": "UTF-8" }} }}
  ]
}}"#
    )
}

/// ② 对照组：与 [`def_declared`] **只差** `left_head` 两个字段。
///
/// 用字符串替换而不是另抄一份 —— 「只差这两个字段」由**构造方式**保证，
/// 不靠人肉比对（抄一份迟早会悄悄漂移）。
fn def_auto() -> String {
    let d = def_declared();
    let out = d.replace(r#", "left_head": "B1""#, "");
    assert_eq!(
        d.matches(r#""left_head": "B1""#).count(),
        2,
        "模板应恰有两处 left_head（左甲 / 左乙）"
    );
    assert!(!out.contains("left_head"), "替换后不该残留 left_head");
    out
}

/// ④ 主格在**下**边：`上左`(1,1) 的左主格指向 `B2`（第 2 行、第 2 列 —— 在右下）。
fn def_master_below() -> String {
    format!(
        r#"{{
  "version": 1,
  "grid": {{ "rows": 3, "cols": 3 }},
  "cells": [
    {{ "row": 1, "col": 1, "text": "上左", "left_head": "B2" }},
    {{ "row": 2, "col": 2, "expr": "ds1.select(region)", "extend_mode": "vertical" }}
  ],
  "datasets": [
    {{ "name": "ds1", "file": {{ "file_name": "{CSV}", "encoding": "UTF-8" }} }}
  ]
}}"#
    )
}

// ---------------------------------------------------------------- 脚手架

/// 库函数直调（同 `reportdef_http.rs` 的口径）。
fn render(def: &str) -> Result<String, String> {
    let req = RenderRequest {
        report: def.as_bytes().to_vec(),
        report_name: "multipiece".to_string(),
        datasets: [(CSV.to_string(), common::fixture(CSV))]
            .into_iter()
            .collect(),
        params: ParamTable::EMPTY,
    };
    render_request(&req).map_err(|e| format!("{e:?}"))
}

fn sheet(def: &str) -> serde_json::Value {
    let text = render(def).expect("模板应渲染成功");
    let j: serde_json::Value = serde_json::from_str(&text).unwrap();
    j["sheets"]["rpx-sheet-1"].clone()
}

/// `cellData[r][c].v` 的文本（空槽 ⇒ `None`）。
fn cell(s: &serde_json::Value, r: usize, c: usize) -> Option<String> {
    let v = &s["cellData"][r.to_string()][c.to_string()]["v"];
    if v.is_null() {
        None
    } else {
        Some(
            v.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string()),
        )
    }
}

/// 第 `c` 列上取值为 `want` 的**行号**（0 起，升序）—— 用来量复制的份数与步长。
///
/// ⚠ 不能用「该列所有有值的行」代替：被拉大的合并格会在每一行都有值，
/// 与复制出来的多份在「有值的行集合」上**同形**。必须按**值**筛。
fn rows_where(s: &serde_json::Value, c: usize, want: &str) -> Vec<usize> {
    let mut out = Vec::new();
    if let Some(obj) = s["cellData"].as_object() {
        let mut rows: Vec<usize> = obj.keys().filter_map(|k| k.parse().ok()).collect();
        rows.sort_unstable();
        for r in rows {
            if cell(s, r, c).as_deref() == Some(want) {
                out.push(r);
            }
        }
    }
    out
}

/// `mergeData` → `(startRow, startColumn, endRow, endColumn)`（0 起闭区间）。
fn merge_rects(s: &serde_json::Value) -> Vec<(u64, u64, u64, u64)> {
    s["mergeData"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            (
                m["startRow"].as_u64().unwrap(),
                m["startColumn"].as_u64().unwrap(),
                m["endRow"].as_u64().unwrap(),
                m["endColumn"].as_u64().unwrap(),
            )
        })
        .collect()
}

// ---------------------------------------------------------------- 判据 1：多片扩展

/// 声明的附属格**随主格复制**：主格 5 个实例 ⇒ `左甲`/`左乙` 各 5 份，
/// 且**不合并**（`mergeData` 为空 —— 这是与「拉大」的分界线）。
#[test]
fn a_declared_piece_is_copied_with_the_master() {
    let s = sheet(&def_declared());

    // 主格在 (1,2)；行带 = 2 行（`左甲` 行 + `左乙` 行）⇒ 实例步长 2。
    assert_eq!(
        rows_where(&s, 0, "左甲"),
        vec![0, 2, 4, 6, 8],
        "左甲应随主格复制 5 份"
    );
    assert_eq!(
        rows_where(&s, 0, "左乙"),
        vec![1, 3, 5, 7, 9],
        "左乙应随主格复制 5 份（缺省它不会被复制，见判据 2）"
    );
    assert_eq!(rows_where(&s, 0, "尾"), vec![10], "表尾被推到 5 个行带之后");

    // 主格的 5 个值按 CSV 顺序落在实例行上。
    for (i, r) in [0usize, 2, 4, 6, 8].iter().enumerate() {
        assert_eq!(cell(&s, *r, 1).as_deref(), Some(REGIONS[i]), "第 {r} 行");
    }

    // ⛔ 关键分界：复制 ≠ 拉大。多片扩展下**不该有任何合并区**。
    assert!(
        merge_rects(&s).is_empty(),
        "声明的附属格是**复制**出来的独立格，不该有合并区：{:?}",
        merge_rects(&s)
    );
}

// ---------------------------------------------------------------- 判据 2：对照组

/// **不声明**且与主格同级 ⇒ 被**拉大**成 5 行合并区，**不复制**。
#[test]
fn without_a_declaration_the_same_level_cell_is_stretched_not_copied() {
    let s = sheet(&def_auto());

    // 拉大：`左甲` 撑成 5 行合并区（0 起 (0,0)-(4,0)），且**只有这一条**。
    assert_eq!(
        merge_rects(&s),
        vec![(0, 0, 4, 0)],
        "同级格应被拉大成 5 行合并区"
    );
    // 拉大后的合并格在 5 行上都有值（所以判据必须按**值**筛，见 `rows_where`）。
    assert_eq!(rows_where(&s, 0, "左甲"), vec![0, 1, 2, 3, 4]);

    // 行带只剩 1 行 ⇒ 主格 5 个实例步长 1。
    assert_eq!(rows_where(&s, 0, "尾"), vec![6]);
    for (i, r) in (0usize..5).enumerate() {
        assert_eq!(cell(&s, r, 1).as_deref(), Some(REGIONS[i]), "第 {r} 行");
    }

    // `左乙`（在主格左边但**不同级**）不复制：只出现一次，被推到合并区之后。
    assert_eq!(
        rows_where(&s, 0, "左乙"),
        vec![5],
        "未声明的左邻格不该被复制"
    );
}

// ---------------------------------------------------------------- 判据 3：缺省附属格

/// 缺省规则：主格**右（下）边的格**天然是它的附属格（教程原话），无需声明。
#[test]
fn the_cell_to_the_right_is_a_subsidiary_by_default() {
    let s = sheet(&def_auto());
    // `右`(1,3) 未声明任何主格，但它在主格 B1 的右边 ⇒ 随主格复制 5 份。
    assert_eq!(
        rows_where(&s, 2, "右"),
        vec![0, 1, 2, 3, 4],
        "右应复制 5 份"
    );
}

// ---------------------------------------------------------------- 判据 4：主格在下方

/// 教程明写：「人为设置时，**可能发生左（上）主格在右（下）边**的情况，而且主格也不一定
/// 和附属格在同一行（列）上。」
///
/// `上左`(1,1) 的左主格指向 `B2`（第 2 行第 2 列 —— **下方且右边**），
/// 引擎必须照认：主格 5 个实例 ⇒ `上左` 5 份。
#[test]
fn a_master_below_and_to_the_right_still_governs() {
    let s = sheet(&def_master_below());
    // 行带 = 2 行（`上左` 行 + 主格行）⇒ 步长 2。
    assert_eq!(
        rows_where(&s, 0, "上左"),
        vec![0, 2, 4, 6, 8],
        "上左应随下方主格复制 5 份"
    );
    for (i, r) in [1usize, 3, 5, 7, 9].iter().enumerate() {
        assert_eq!(cell(&s, *r, 1).as_deref(), Some(REGIONS[i]), "第 {r} 行");
    }
    assert!(merge_rects(&s).is_empty(), "复制不应产生合并区");
}

// ---------------------------------------------------------------- 判据 5：三条硬约束

/// 教程 `dpkz101`「人为改变主格规则」的三条硬约束 —— 装载期必须拦下 ①/③，
/// ② 由引擎拦（`heads/find.rs` 分支 A 的自含判定）。
///
/// ⚠ 断言的是**错误文本里的 JSON 路径**（前端据此定位），不是「有没有报错」——
/// 后者对任何原因的失败都是绿的。
#[test]
fn the_three_master_cell_constraints_are_enforced() {
    let cases: [(&str, &str, &str); 4] = [
        // ③ 横向扩展格不允许有左主格
        (
            r#"{"row":1,"col":2,"expr":"ds1.select(region)","extend_mode":"vertical"},
               {"row":2,"col":1,"text":"x","extend_mode":"horizontal","left_head":"B1"}"#,
            "$.cells[1].left_head",
            "横向扩展格不允许有左主格",
        ),
        // ③ 纵向扩展格不允许有上主格
        (
            r#"{"row":1,"col":2,"expr":"ds1.select(region)","extend_mode":"horizontal"},
               {"row":2,"col":1,"text":"x","extend_mode":"vertical","top_head":"B1"}"#,
            "$.cells[1].top_head",
            "纵向扩展格不允许有上主格",
        ),
        // ① 左主格必须是纵向扩展格
        (
            r#"{"row":1,"col":3,"expr":"ds1.select(region)","extend_mode":"horizontal"},
               {"row":2,"col":1,"text":"x","left_head":"C1"}"#,
            "$.cells[1].left_head",
            "左主格必须是纵向扩展格",
        ),
        // ① 上主格必须是横向扩展格
        (
            r#"{"row":1,"col":1,"expr":"ds1.select(region)","extend_mode":"vertical"},
               {"row":2,"col":2,"text":"x","top_head":"A1"}"#,
            "$.cells[1].top_head",
            "上主格必须是横向扩展格",
        ),
    ];
    for (cells, path, expect) in cases {
        let def = format!(
            r#"{{"grid":{{"rows":3,"cols":3}},"cells":[{cells}],
                "datasets":[{{"name":"ds1","file":{{"file_name":"{CSV}","encoding":"UTF-8"}}}}]}}"#
        );
        let e = render(&def).unwrap_err();
        assert!(e.contains(path), "应指到 {path}，实得：{e}");
        assert!(e.contains(expect), "应含「{expect}」，实得：{e}");
    }

    // ② 成环：**引擎**拦（不是装载层 —— 缺省推导不可能成环，只有人为设置会）。
    let cyc = r#"{"grid":{"rows":3,"cols":3},
        "cells":[{"row":1,"col":1,"text":"a","left_head":"B1"},
                 {"row":1,"col":2,"text":"b","left_head":"A1"}]}"#;
    let e = render(cyc).unwrap_err();
    assert!(e.contains("自声明成环"), "② 应由引擎报成环，实得：{e}");
}
