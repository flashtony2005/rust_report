//! M5 §6.3 残账（P1 多键并列 / P2 过滤非布尔 / P4 `group@b`）的钉子。
//!
//! 三条夹具已进主对拍（`engine_tiesort` / `engine_rowtest` / `engine_groupb`），
//! 本文件补主对拍**看不到**的三件事：
//!
//! 1. **消息文本**：主对拍只比 `(class, chain)`，产品 `Row.test` 的
//!    「记录过滤条件应该返回布尔值」（`Row.java:88-94`，bundle 键 `error.Row.test`）
//!    不在比较范围 ⇒ 在这里单独钉住引擎侧措辞。
//! 2. **非空转守卫**：`engine_tiesort` 若退化成"两列同序"，"稳定"就没有承重；
//!    这里先证明排序通道本身有效（对照列 = 1/3/5/7/9），再钉并列列 = 输入序。
//! 3. **`@b` 的等价域**：产品的二分快速路径**只在数据按该列升序时**等价于线性过滤。
//!    夹具只覆盖等价域；无序域的偏差见 `docs/spec-m5.md` §6.3 ③。
//!
//! 事实源：`tests/fixtures/out/m5r_probe.log`（`ProbeM5Rest` 12 例产品实跑）。

mod common;

use std::path::PathBuf;

use rpx_engine::{EngineError, Value};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn calc(name: &str) -> rpx_engine::CalcGrid {
    let rd = common::open_json_fixture(name);
    let ds = rpx_engine::dataset::load_datasets(&fixtures_dir(), &rd)
        .unwrap_or_else(|e| panic!("{name}: load_datasets 失败：{e}"));
    let fx = common::load(name);
    let params = rpx_engine::ParamTable::from_iter(
        fx.params.iter().map(|p| (p.name.clone(), p.value.clone())),
    );
    rpx_engine::calculate_with(&rd, &ds, &params)
        .unwrap_or_else(|e| panic!("{name}: calculate 失败：{e}"))
}

fn value(g: &rpx_engine::CalcGrid, r: i32, c: i32) -> Value {
    g.get(r, c)
        .map(|cell| cell.value.clone())
        .unwrap_or(Value::Null)
}

/// **P2**：过滤条件非布尔 ⇒ 产品 `Row.test` 的原文「记录过滤条件应该返回布尔值」。
///
/// 产品完整消息还带单元格包装（`单元格A2中有错误: 表达式：ds1.select(id, 5)中: …`），
/// 那是**格级**外壳、不是 `Row.test` 本身；本仓没有这层外壳，只钉内层措辞。
#[test]
fn non_boolean_filter_carries_the_product_row_test_wording() {
    let rd = common::open_json_fixture("engine_rowtest");
    let ds = rpx_engine::dataset::load_datasets(&fixtures_dir(), &rd).expect("dataset");
    let err = rpx_engine::calculate_with(&rd, &ds, &rpx_engine::ParamTable::EMPTY)
        .err()
        .expect("过滤条件是常量 5，必须报错");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("记录过滤条件应该返回布尔值"),
        "错误文本必须含产品 error.Row.test 的原文，实得：{msg}"
    );
    // 守卫：错误类型不能是"碰巧别的错"，必须就是非布尔过滤这条。
    assert!(
        matches!(err, EngineError::UnsupportedExpr { .. }),
        "期望 UnsupportedExpr（as_bool 出口），实得 {err:?}"
    );
}

/// **P1**：多键排序**全键相等**时产出 = 输入序（产品手写归并的稳定性）。
///
/// 产品 `SortDsValue.sort:384-401`：先按首键 `Collections.sort`（稳定），再对每个
/// 首键相等的连续段按次键重排 —— 全键相等 ⇒ 每趟比较都为 0 且都稳定 ⇒ 输入序。
/// 探针 `ProbeM5Rest` 实测 7/3/9/1/5（乱序输入，未被"顺手排一下"）。
#[test]
fn tied_sort_keys_keep_the_input_order() {
    let g = calc("engine_tiesort");
    let tie: Vec<Value> = (2..=6).map(|r| value(&g, r, 1)).collect();
    let sorted: Vec<Value> = (12..=16).map(|r| value(&g, r, 4)).collect();

    // 守卫：对照带（`;id`）必须真的是升序，否则"排序通道坏了"会让本钉子空转 ——
    // 那时两列同序，"稳定"根本无从谈起。
    assert_eq!(
        sorted,
        vec![
            Value::Int(1),
            Value::Int(3),
            Value::Int(5),
            Value::Int(7),
            Value::Int(9)
        ],
        "对照带必须是 1/3/5/7/9（排序通道有效）"
    );
    assert_eq!(
        tie,
        vec![
            Value::Int(7),
            Value::Int(3),
            Value::Int(9),
            Value::Int(1),
            Value::Int(5)
        ],
        "全键相等时必须保持输入序；任何非稳定排序或「再按值排一次」都会打乱它"
    );
}

/// **P4**：`@b` 二分快速路径与线性过滤在**升序前提**下同值。
///
/// 夹具 `engine_groupb` 的数据按 `g` 升序（A A B B B）——
/// 探针实测二分与线性同为 21（B）/ 4（A）。
///
/// ⛔ **无序域不复刻**：产品在无序数据上返回"二分命中点所在的连续段"
/// （探针：无序 `sum@b(id, g=="B")` = **9**，线性 = 21）。本仓一律走线性，
/// 在该域与产品**有意偏差**，理由见 `docs/spec-m5.md` §6.3 ③。
#[test]
fn group_binary_option_matches_linear_when_the_column_is_sorted() {
    let g = calc("engine_groupb");
    assert_eq!(value(&g, 2, 1), Value::Int(21), "sum@b(id, g==B)");
    assert_eq!(value(&g, 2, 2), Value::Int(21), "sum(id, g==B) 线性");
    assert_eq!(value(&g, 2, 3), Value::Int(4), "sum@b(id, g==A)");
    // 守卫：两个值必须同源可辨（B 组 3 行 = 21，A 组 2 行 = 4），否则同值可能是巧合。
    assert_ne!(
        value(&g, 2, 1),
        value(&g, 2, 3),
        "两组同值时本钉子无法分辨是否真的过滤了"
    );
}
