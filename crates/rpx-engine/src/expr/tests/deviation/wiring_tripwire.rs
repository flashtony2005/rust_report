//! 接线绊线：本切片**刻意不消费**函数优先级表，这里把"当前真实树形"钉死。
//!
//! 背景：`names.rs` 的 `FUN_PRIORITY_*` 是真的产品事实，但 Pratt parser 把所有调用
//! 当原子（19），所以没人查它 —— 独立验证方称之为"孤儿表 + 永真式单测"，要求处置。
//! 处置见 `names.rs` 的 ⛔ 横幅。
//!
//! 本文件**不是**重复测那张表（那是永真式），而是钉住 **parser 实际产出的树形**：
//! 将来若有人把优先级表接进 parser，下面两条断言会失败，从而强制一次显式决策
//! ——「要不要连产品的 orphaning 一起复刻」。

use super::*;

/// 优先级低于前缀的函数紧跟其后时，**我们保留前缀**（Pratt 只会往上挂）。
///
/// 产品真机（证据：`tests/fixtures/probe_fun_priority_out.utf8.txt`）在同样形状下
/// **把前缀孤立丢弃** —— `Node.setLeft` 是空实现（`Node.java:64-65`），
/// `Expression.java:442-450` 的右脊下探在前驱优先级更高时得到 `parent == null`，
/// 于是 `home` 被覆盖、Function 又不存 left ⇒ 前驱子树丢失。
#[test]
fn prefix_operator_before_a_low_priority_call_is_kept_by_our_parser() {
    // 我们：`-(lefthead(A1))` = Unary(Neg, Call)
    let e = parse("-lefthead(A1)").unwrap();
    let Expr::Unary { op: UnOp::Neg, rhs } = &e else {
        panic!("我们保留了前缀：顶层应是 Unary(Neg, _)，实际 {e:?}")
    };
    let Expr::Call(c) = &**rhs else {
        panic!("右操作数应是调用")
    };
    assert_eq!(c.name, "lefthead");
    assert_eq!(
        c.kind,
        CallKind::Regular,
        "lefthead 在普通函数表里且未覆写 isExtended"
    );

    // 对照：`-sum(A1)` 两边一致（Sum=19 > 前缀 18 ⇒ 前缀正常挂上），所以它不是差异点
    let s = parse("-sum(A1)").unwrap();
    let Expr::Unary {
        op: UnOp::Neg,
        rhs: s_rhs,
    } = &s
    else {
        panic!("对照组顶层应是 Unary")
    };
    let Expr::Call(sc) = &**s_rhs else {
        panic!("对照组右操作数应是调用")
    };
    assert_eq!(sc.name, "sum");
}

/// 把"产品丢弃前缀、我们保留"这个差异**显式钉住**，免得将来有人以为两边一致。
#[test]
fn the_known_divergence_from_the_products_orphan_behaviour_is_pinned() {
    // 产品：`-mcorder(1)` → root=McOrder(16)，前缀被丢；我们：Unary(Neg, Call(mcorder))
    let e = parse("-mcorder(1)").unwrap();
    assert!(
        matches!(&e, Expr::Unary { op: UnOp::Neg, rhs } if matches!(&**rhs, Expr::Call(c) if c.name == "mcorder")),
        "我们保留 Neg；产品在这条上丢失它 —— 若此断言失败说明 parser 被改动，必须重读 names.rs 的 ⛔ 横幅"
    );
    // 且 is_extend 两边都是 false ⇒ 对 M2 扩展算法零影响（这三个函数类都没覆写 isExtended）
    assert!(
        !e.is_extend(),
        "该形状下 is_extend 恒 false，不受接线与否影响"
    );
    // 同一形状里 16 与 19 的差别不影响 is_extend —— 这也是当初可以不接线的关键理由
    assert!(!e.is_extend() && !parse("mcorder(1)").unwrap().is_extend());
}

// 子模块声明放在文件末尾：它的 `//!` 模块文档必须紧贴文件开头。
