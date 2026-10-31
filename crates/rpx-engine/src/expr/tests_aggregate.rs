//! M2-12 `avg` / `max` / `min` 的求值器单测（自 `tests_eval.rs` 拆出）。
//!
//! 拆出的唯一理由是 G4 行数门禁：`tests_eval.rs` 已 296 行，并进去就破 500。
//! 复用同一个 [`Fake`](super::tests_eval)、[`ds_vals`] 之外的假网格；
//! `tests_eval` 是 `expr` 的私有子模块 ⇒ 这里用 `super::tests_eval::Fake`。
//!
//! 事实源：`Average.java:100-125` / `Max.java:100-121` / `Min.java:100-121`
//! （`re_work/src/reportlite/03-model/.../expression/function/`）。
//! 三者与 `sum` 同在 [`crate::expr::dsfn`]，**叶子路径同构、聚合路径各不同**，
//! 故每条差别都单开一条用例。

use super::tests_eval::Fake;
use crate::arena::CellId;
use crate::expr::ds::DataSet;
use crate::expr::eval::evaluate;
use crate::expr::parse;
use crate::value::Value;
// ═══════════════════════════ M2-12：avg / max / min ═══════════════════════════════
//
// 事实源：`Average.java:100-125` / `Max.java:100-121` / `Min.java:100-121`
// （reportlite 树）。三者与 `sum` 同在 [`super::dsfn`]，**叶子路径同构、
// 聚合路径各不同**，故每条差别都单开一条用例。

/// 任意值列的数据集（`Fake::ds3` 只造 Int，这里要覆盖 Text/Bool/Null）。
fn ds_vals(name: &str, col: &str, vals: &[Value]) -> DataSet {
    DataSet::new(name, &[col], vals.iter().map(|v| vec![v.clone()]).collect())
}

/// ① **空表 → `Null`**，不是 `Double(0.0)` —— 与 `sum` 唯一的结构性差别
///    （`Average.java:112-114` 的 `count == 0 → return null`）。
///    这条是"顺手统一成 sum 的形状"最可能踩的坑，必须有钉子。
#[test]
fn avg_of_empty_list_is_null_but_sum_is_double_zero() {
    let mut f = Fake::new();
    f.push_ds(Fake::ds3("ds1", "qty", &[]));
    assert!(matches!(
        evaluate(&parse("avg(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::Null
    ));
    assert!(matches!(
        evaluate(&parse("sum(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::Double(_)
    ));
}

/// ② 分母口径：默认**扣掉 null**，带 `@a` 时**不扣**
///    （`Average.java:108` 的 `option.indexOf(97) != -1` → `:125`）。
#[test]
fn avg_denominator_excludes_null_unless_option_a() {
    let mut f = Fake::new();
    f.push_ds(ds_vals(
        "ds1",
        "qty",
        &[Value::Int(10), Value::Null, Value::Int(30)],
    ));
    let mut f2 = Fake::new();
    f2.push_ds(ds_vals(
        "ds1",
        "qty",
        &[Value::Int(10), Value::Null, Value::Int(30)],
    ));
    // 40 / 2 = 20.0（null 不计）
    assert!(matches!(
        evaluate(&parse("avg(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::Double(d) if (d - 20.0).abs() < 1e-9
    ));
    // 40 / 3 = 13.333…（@a 把 null 也计入分母）
    assert!(matches!(
        evaluate(&parse("avg@a(ds1.select(qty))").unwrap(), &mut f2).unwrap(),
        Value::Double(d) if (d - 40.0 / 3.0).abs() < 1e-9
    ));
}

/// ③ 全空元素 → `Double(0.0)`；且该判定**必须先于除零**
///    （`Average.java:122-125`：此时 `count - nullCount` 恰为 0）。
#[test]
fn avg_of_all_null_list_is_double_zero_and_does_not_divide_by_zero() {
    let mut f = Fake::new();
    f.push_ds(ds_vals("ds1", "qty", &[Value::Null, Value::Null]));
    assert!(matches!(
        evaluate(&parse("avg(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::Double(d) if d == 0.0
    ));
}

/// ④ `Variant2.divide` 无整型分支 ⇒ `avg` 恒为 `Double`
///    （`num_div` 的模块文档，`Average.java:125`）。
#[test]
fn avg_always_widens_to_double_like_divide() {
    let mut f = Fake::new();
    f.push_ds(Fake::ds3("ds1", "qty", &[100, 200, 150]));
    assert!(matches!(
        evaluate(&parse("avg(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::Double(d) if (d - 150.0).abs() < 1e-9
    ));
}

/// ⑤ 标量透传：非 List 实参原样返回
///    （`Average.java:120-122` / `Max:115-117` / `Min:115-117`）。
///    ⚠ 这条**不是**"顺手统一成 List 也能过"：`avg` 的标量路径直接 `return result1`
///    而 List 路径必过 `Variant2.divide` ⇒ `avg(Int(7))` 是 `Int(7)`，不是 `Double(7.0)`。
#[test]
fn avg_max_min_pass_a_scalar_through_unchanged() {
    let mut f = Fake::new();
    // current 指向 A1 自身 ⇒ 解析 `A1` 命中本格（同 `arithmetic_keeps_int_type` 的接线）。
    let a1 = f.put("A1", 16, Value::Int(7));
    f.set_cur(a1);
    for src in ["avg(A1)", "max(A1)", "min(A1)", "sum(A1)"] {
        assert!(
            matches!(
                evaluate(&parse(src).unwrap(), &mut f).unwrap(),
                Value::Int(7)
            ),
            "{src} 应原样透传 Int(7)，而不是 Double(7.0)"
        );
    }
}

/// ⑥ max/min 取极值；空表与全空表 → `Null`（`result` 初值 null 从未被赋值）。
#[test]
fn max_min_pick_extremes_and_empty_is_null() {
    let mut f = Fake::new();
    f.push_ds(Fake::ds3("ds1", "qty", &[30, 10, 50, 20]));
    assert!(matches!(
        evaluate(&parse("max(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::Int(50)
    ));
    assert!(matches!(
        evaluate(&parse("min(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::Int(10)
    ));

    let mut g = Fake::new();
    g.push_ds(Fake::ds3("ds1", "qty", &[]));
    assert!(matches!(
        evaluate(&parse("max(ds1.select(qty))").unwrap(), &mut g).unwrap(),
        Value::Null
    ));
    let mut h = Fake::new();
    h.push_ds(ds_vals("ds1", "qty", &[Value::Null, Value::Null]));
    assert!(matches!(
        evaluate(&parse("min(ds1.select(qty))").unwrap(), &mut h).unwrap(),
        Value::Null
    ));
}

/// ⑦ **产品原文的不对称**：Max 用解包后的标量 `result = o`，Min 用**原始元素**
///    `result = list.get(i)`（`Min.java:115`）。只在"元素本身是非空 `List`"时分叉。
///    ⛔ 这条钉的是**复刻**，不是正确性 —— 修平之前先拿产品侧证据。
#[test]
fn min_keeps_the_raw_list_element_where_max_unwraps() {
    let mut f = Fake::new();
    // 每行一列，值本身是 List ⇒ select 得到 List[List(5), List(3)]
    f.push_ds(ds_vals(
        "ds1",
        "qty",
        &[
            Value::List(vec![Value::Int(5)]),
            Value::List(vec![Value::Int(3)]),
        ],
    ));
    // max：解包 ⇒ Int(5)；min：不解包 ⇒ List([Int(3)])
    assert!(matches!(
        evaluate(&parse("max(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::Int(5)
    ));
    assert!(matches!(
        evaluate(&parse("min(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::List(_)
    ));
}

/// ⑧ `Text-Text` 走**产品字典序**（`ChineseComparator`）而不是码点序 —— 复刻与坑见
///    `docs/m2-collation.md`。参照值：阿(294) < 一(18183) < 张(19926)。
///    另钉一条"码点序与产品序相反"：一 U+4E00 < 丁 U+4E01，但产品序 丁(3322) < 一(18183)。
///    本条取代旧的"`Text-Text` 应显式报错"失效断言（那断言基于"rule.txt 不在仓库"的错误认知）。
#[test]
fn text_text_uses_product_collation_order_not_code_point_order() {
    use crate::expr::num::aggregate_cmp;
    use std::cmp::Ordering;
    let t = |s: &str| Value::Text(s.into());
    assert_eq!(aggregate_cmp(&t("阿"), &t("一")).unwrap(), Ordering::Less);
    assert_eq!(
        aggregate_cmp(&t("张"), &t("一")).unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        aggregate_cmp(&t("一"), &t("丁")).unwrap(),
        Ordering::Greater
    );

    // 端到端：`max`/`min` 也走同一比较（不是只改了单测入口）。
    let mut f = Fake::new();
    f.push_ds(ds_vals(
        "ds1",
        "s",
        &[Value::Text("张".into()), Value::Text("阿".into())],
    ));
    assert!(matches!(
        evaluate(&parse("max(ds1.select(s))").unwrap(), &mut f).unwrap(),
        Value::Text(v) if v == "张"
    ));
    let mut g = Fake::new();
    g.push_ds(ds_vals(
        "ds1",
        "s",
        &[Value::Text("张".into()), Value::Text("阿".into())],
    ));
    assert!(matches!(
        evaluate(&parse("min(ds1.select(s))").unwrap(), &mut g).unwrap(),
        Value::Text(v) if v == "阿"
    ));
}

/// ⑨ `Bool-Bool` 可排序（`compare2:453-455` → `compare(boolean,boolean)`，false < true）。
#[test]
fn bool_ordering_is_false_less_than_true() {
    let mut f = Fake::new();
    f.push_ds(ds_vals(
        "ds1",
        "b",
        &[Value::Bool(false), Value::Bool(true), Value::Bool(false)],
    ));
    assert!(matches!(
        evaluate(&parse("max(ds1.select(b))").unwrap(), &mut f).unwrap(),
        Value::Bool(true)
    ));
    assert!(matches!(
        evaluate(&parse("min(ds1.select(b))").unwrap(), &mut f).unwrap(),
        Value::Bool(false)
    ));
}

/// ⑩ DS 形态（`ds.avg/max/min(col)`）Root 态聚合 + Row 态逐行
///     （与 `sum` 同构；reportlite 树里没有这三个的 DS 包装类，
///     故 Row 态沿用 `DSSum.java:46` 的 `return cur;`）。
#[test]
fn ds_qualified_avg_max_min_dual_mode() {
    let mut f = Fake::new();
    f.push_ds(Fake::ds3("ds1", "qty", &[100, 200, 150]));
    // Root 态
    assert!(matches!(
        evaluate(&parse("ds1.avg(qty)").unwrap(), &mut f).unwrap(),
        Value::Double(d) if (d - 150.0).abs() < 1e-9
    ));
    assert!(matches!(
        evaluate(&parse("ds1.max(qty)").unwrap(), &mut f).unwrap(),
        Value::Int(200)
    ));
    assert!(matches!(
        evaluate(&parse("ds1.min(qty)").unwrap(), &mut f).unwrap(),
        Value::Int(100)
    ));
    // Row 态：指针停在第 2 行 ⇒ 返回该行值（与 select/sum 同形）
    f.ds_set_row(0, 1);
    assert!(matches!(
        evaluate(&parse("ds1.max(qty)").unwrap(), &mut f).unwrap(),
        Value::Int(200)
    ));
}

/// ⑪ §M2-12 顺带修正的钉子：`sum` 原先**没过 `getSingleValue`** ⇒
///    `sum(List[List(1)])` 会报 `UnsupportedExpr`，而产品返回 `1`
///    （`Sum.java:124-126`）。现在与 `merge::same_value` 共用
///    [`crate::value::single_value`]，两处不会再漂移。
#[test]
fn sum_list_unwraps_single_value_like_java() {
    let mut f = Fake::new();
    f.push_ds(ds_vals(
        "ds1",
        "qty",
        &[
            Value::List(vec![Value::Int(1)]),
            Value::List(vec![Value::Int(2)]),
        ],
    ));
    assert!(matches!(
        evaluate(&parse("sum(ds1.select(qty))").unwrap(), &mut f).unwrap(),
        Value::Int(3)
    ));
}

/// ⑫ **区域 List 形态的「null 槽位」语义**（`Min.java:157-165`，`Max.java` 同形）。
///
/// 产品在累积处写 `result = Variant2.getSingleValue(obj)`：元素是**空 List** 时
/// `getSingleValue` 返回 `null` ⇒ `result` 变 **Java null**，下一轮 `result != null`
/// 判假 ⇒ 走「无条件重新初始化」，**不是**拿 null 去比较。
///
/// 序列 `[100, 空List, 50]` ⇒ 产品 `min` = **50**。
/// ⛔ 反例：若把 null 槽位记成 `Some(Value::Null)`，下一轮会走比较分支 ——
/// `compare2(null, 50) = -1`（`Variant2.java:446-448`）而 `Min` 的
/// `if (compare(result, obj) <= 0) continue;` 判**真** ⇒ 保持 null ⇒ 返回 `null`。
/// 本仓曾如此实现，2026-10-14 按源码改成 `Option::None`（`eval.rs` 的注释有推导）。
///
/// 可达条件（元素是「空 List 或首元素为 null 的 List」）**产品夹具无此形态**
/// ⇒ 本条是**合成钉子**，与 `docs/m2-engine-design.md` §14.6 的缺口声明配套。
#[test]
fn region_min_reinitializes_after_a_null_slot_instead_of_comparing() {
    let mut f = Fake::new();
    let b2 = f.put("B2", 16, Value::Int(100));
    let b3 = f.put("B3", 16, Value::List(Vec::new()));
    let b4 = f.put("B4", 16, Value::Int(50));
    // 区域遍历按计算态坐标收集（`Link._$1:83` 的 getCell(i,j)），
    // 夹具侧由 `engine_listrange` 覆盖同一路径；这里只补"空 List 元素"这一形态。
    f.set_pos(b2, 2, 2);
    f.set_pos(b3, 3, 2);
    f.set_pos(b4, 4, 2);
    f.reg_top(CellId::ROOT, b2);
    f.reg_top(CellId::ROOT, b4);
    // 端点解析与 `@` 都以 current 为基准（`saveCurrent` 发生在区域入口
    // `Sum.java:150`，由 `region_loop` 自己调，这里只需把 current 摆对）。
    f.set_cur(b2);
    let e = parse("min(@:[B2:B4])").unwrap();
    assert_eq!(evaluate(&e, &mut f).unwrap(), Value::Int(50));
}
