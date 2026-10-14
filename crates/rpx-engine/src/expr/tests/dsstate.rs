//! `is_ds_function()` 与 `get_state()` 的真值表 + 前提钉子。
//!
//! 出处、类层次与"为什么两条链必须分开"见 [`crate::expr::dsstate`] 的模块文档。
//! 本文件只留断言；每条都挂着 `file:line`。

use super::*;

/// 解析并取 `is_ds_function()`。
fn dsf(src: &str) -> bool {
    parse(src)
        .unwrap_or_else(|e| panic!("解析 `{src}` 失败：{e}"))
        .is_ds_function()
}

/// 解析并取 `get_state()`。
fn st(src: &str) -> u8 {
    parse(src)
        .unwrap_or_else(|e| panic!("解析 `{src}` 失败：{e}"))
        .get_state()
}

/// `is_ds_function()` 的真值表，逐条对照各类的覆写点。
#[test]
fn ds_function_truth_table_mirrors_the_java_overrides() {
    // DSFunction.isDSFunction() = true（DSFunction.java:25-27）
    assert!(dsf("ds1.select(qty)"));
    assert!(dsf("ds2.count()"));
    // DSVariable.isDSFunction() = true（DSVariable.java:54-56）
    assert!(dsf("ds1.qty"));
    // UnknownSymbol.isDSFunction() = true（UnknownSymbol.java:39-41）—— DS 参数里的裸列名
    assert!(dsf("qty"));
    // Constant / CSVariable / Operator（两侧都非 DS）⇒ false（Node.java:103-105）
    assert!(!dsf("6*1"));
    assert!(!dsf("B2*2"));
    assert!(!dsf("B2*B3"));
    // Function 向参数透传（Function.java:55-60）
    assert!(dsf("sum(ds1.select(qty))"));
    // Operator 任一侧为真（Operator.java:91-96）
    assert!(dsf("B2+ds1.select(qty)"));
    // ValueList extends Function ⇒ 逐项
    assert!(dsf("list(ds1.select(qty))"));
    assert!(!dsf("list(1,2)"));
}

/// ⛔ **头号回归**：两条链必须相反 —— 本谓词真、`is_extend()` 假。
///
/// 若有人把两者互相近似（本项目实测最容易犯的一步），这条立刻红。
/// 差距有值级后果：`is_ds_function` 触发 DS 指针重算（`ExtNormalCell.java:648`），
/// `is_extend` 决定包不包成 `ExtExtendCell`（`checkExtend:510`）。
#[test]
fn ds_function_and_extend_disagree_on_the_same_expression() {
    let e = parse("sum(ds1.select(qty))").unwrap();
    assert!(e.is_ds_function(), "`sum(ds1.select(qty))` 必须是 DS 函数");
    assert!(
        !e.is_extend(),
        "同一条表达式必须**不**驱动扩展 —— 两条链相反"
    );

    // 反向的一对：`B2*2` 两条都假；`ds1.select(qty)` 两条都真。
    assert!(!dsf("B2*2") && !ext("B2*2"));
    assert!(dsf("ds1.select(qty)") && ext("ds1.select(qty)"));

    // ⚠ 与 `is_extend` 的不对称：`SpecialFunction` 的 super 是 Function
    //   （`SpecialFunction extends Function`，类声明已核）⇒ `{}` 的参数**会被查**。
    let sel = parse("{ds1.select(qty)}").unwrap();
    assert!(
        sel.is_ds_function(),
        "`{{...}}` 的参数经 super(Function) 透传 ⇒ 真"
    );
    assert!(!sel.is_extend(), "而 is_extend 只看 left（恒 null）⇒ 假");
}

/// ⛔ **前提钉子**：本 crate 支持的表达式，`get_state()` **恒为 4**。
///
/// 依据不是"没实现"，而是源码结构：`Node` 默认 4（`Node.java:107-109`），
/// `Function`/`Operator`/`SpecialFunction` 三层都只是**取大 + 下限 4**，
/// 且叶子（`Constant`/`CSVariable`/`DSVariable`/`UnknownSymbol`）**全都 `extends Node`
/// 且未覆写**。所以只有显式覆写 `getState` 的函数（`page()`/`cellRowNo()`/`getCell()`/
/// `Graph` 家族）才能产出 > 4，而 12 个引擎夹具一个都没用。
///
/// ⚠ **后果**：`ExtNormalCell.calculate:637` 的 `engine.state > cs.getState()` 恒假
/// ⇒ **state 5/6/8 的挂起路径在现有夹具上恒不可达**。这条前提一破，
/// 通道 5/6 的另两支（special 队列 / 级联失效）就必须先实现再谈对拍 ——
/// 所以它必须是钉子而不是注释。夹具级的全量版本在
/// `crates/rpx-engine/tests/expr_state_tripwire.rs`。
#[test]
fn get_state_is_four_for_every_expression_this_slice_supports() {
    for src in [
        "B2",
        "$B2",
        "@",
        "6*1",
        "B2*2",
        "B2+1",
        "ds1.select(qty)",
        "ds1.qty",
        "ds1.sum(qty)",
        "ds1.count()",
        "ds2.count()",
        "sum(B2:B4)",
        "sum(ds1.select(qty))",
        "list(1,2)",
        "[1:2]",
        "{B2}",
        "-B2",
        "not B2",
        "qty",
    ] {
        assert_eq!(
            st(src),
            4,
            "`{src}` 的 state 应为 4（见本用例的说明与出处表）"
        );
    }
}
