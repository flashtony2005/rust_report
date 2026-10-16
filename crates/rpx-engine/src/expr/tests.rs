//! [`crate::expr`] 的单元测试。
//!
//! 每条断言都挂着来源：`file:line`（产品源码）、夹具名（`tests/fixtures/engine_*.expected.json`）
//! 或真值表（`docs/m2-engine-design.md` §13.3）—— 没有来源的断言不许留在这里。

use super::*;
use crate::params::ParamTable;
use crate::value::Value;

/// 解析并取 `is_extend()`。解析失败直接 panic（测试里就该炸）。
fn ext(src: &str) -> bool {
    parse(src)
        .unwrap_or_else(|e| panic!("解析 `{src}` 失败：{e}"))
        .is_extend()
}

/// 带参数表解析并取 `is_extend()`（M3-01：参数引用在建树期内联成 `Const(值)`，
/// 所以"参数驱动扩展"必须带表才能观察到 —— 与 `engine_param` 夹具的注入对应）。
fn ext_with(src: &str, params: &[(&str, Value)]) -> bool {
    let table = ParamTable::from_iter(params.iter().map(|(n, v)| ((*n).to_string(), v.clone())));
    parse_with(src, &table)
        .unwrap_or_else(|e| panic!("解析 `{src}` 失败：{e}"))
        .is_extend()
}

/// §13.3 真值表的核心四条 + team-lead 点名的五条。
#[test]
fn is_extend_truth_table_core() {
    // DSSelect.isExtended() = true（常返回 true，不看参数内容）—— dsfunction/DSSelect.java:23
    assert!(ext("ds1.select(qty)"), "`ds1.select(x)` 必须为 true");
    // Multiply（Operator）两侧都不扩展 ⇒ false（Operator.java:51-56）
    assert!(!ext("B2*2"), "`B2*2` 必须为 false");
    // Sort / To / SplitString 显式覆写 isExtended()=true —— function/Sort.java:19 等
    assert!(ext("sort(ds1.select(qty))"), "`sort(...)` 必须为 true");
    assert!(ext("to(1,10)"), "`to(...)` 必须为 true");
    assert!(ext("split(name,';')"), "`split(...)` 必须为 true");
    assert!(
        ext("sublist(ds1.select(qty),0,2)"),
        "`sublist(...)` 必须为 true"
    );
    // Constant(is List) = true（Constant.java:22-24）
    let list = Expr::Const(Value::List(vec![Value::Int(1), Value::Int(2)]));
    assert!(list.is_extend(), "`Constant(List)` 必须为 true");
    // ⛔ 反例（本切片的**头号回归**）：Function 不向参数透传 isExtended
    assert!(
        !ext("sum(ds1.select(qty))"),
        "`sum(ds1.select(qty))` 必须为 false —— isDSFunction 会说是 true，两条链相反"
    );
}

/// ⛔ 头号反例的**机制**：外层 `Sum` 是 Regular、内层 `DSSelect` 是 DsExtend ——
/// 但 `Function` 不把参数的 `isExtended()` 透传上来（`Function.java:17-18` 未覆写）。
#[test]
fn outer_regular_function_does_not_inherit_parameter_extendability() {
    let e = parse("sum(ds1.select(qty))").unwrap();
    let Expr::Call(outer) = &e else {
        panic!("外层应为 Call");
    };
    assert_eq!(outer.qualifier, None);
    assert_eq!(outer.name, "sum");
    assert_eq!(
        outer.kind,
        CallKind::Regular,
        "裸 `sum` 是常规 Sum（FunctionLib.java:149 优先于 DS）"
    );
    let Expr::Call(inner) = &outer.args[0] else {
        panic!("内层应为 Call");
    };
    assert_eq!(inner.qualifier.as_deref(), Some("ds1"));
    assert_eq!(inner.name, "select");
    assert_eq!(inner.kind, CallKind::DsExtend);
    assert!(inner.kind.is_extend(), "内层单独看是可扩展的");
    assert!(!e.is_extend(), "但外层不透传 ⇒ 整条表达式不可扩展");
}

/// `plot` 在**两张表里都有**：裸调用普通表优先 ⇒ false；`ds.` 限定才是 DSPlot ⇒ true。
#[test]
fn bare_ds_lookalike_name_resolves_to_the_regular_function() {
    for (src, want) in [
        ("plot(qty)", false),    // 常规 Plot（FunctionLib.java:144）
        ("ds1.plot(qty)", true), // DSPlot（FunctionLib.java:118 + DSPlot.java:21）
        ("sum(qty)", false),     // 常规 Sum
        ("ds1.sum(qty)", false), // DSSum（未覆写 → false）
        ("count()", false),      // 常规 Count
        ("ds2.count()", false),  // DSCount（未覆写 → false）
        ("max(a,b)", false),
        ("ds1.max(qty)", false),
    ] {
        assert_eq!(ext(src), want, "`{src}` 的 is_extend 应为 {want}");
    }
}

/// 裸 DS 名不是 DS 函数（`ds` 恒 null，`ExtNormalCell.java:498-504`）⇒ UnknownSymbol ⇒ false。
#[test]
fn bare_ds_only_names_are_not_ds_calls() {
    for src in ["select(qty)", "group(qty)", "filtercross(qty)"] {
        assert!(!ext(src), "裸 `{src}` 不该被当成 DS 函数");
    }
    // `ds1.zzz(x)`：名字不在 DS 表 ⇒ createDSNode 返回 null ⇒ UnknownSymbol
    assert!(!ext("ds1.zzz(x)"), "名字不在 DS 表 ⇒ UnknownSymbol");
}

/// `Link`（`:`）是唯一覆写成 false 的 Operator（`operator/Link.java:100-102`）。
#[test]
fn link_operator_is_false_even_though_it_is_an_operator() {
    assert!(!ext("B2:B4"));
    assert!(!ext("sum(B2:B4)"), "Sum 不覆写 + Link 恒 false ⇒ false");
    // 对照：同样结构的 `,`（Comma 是 Operator，未覆写）会透传两侧
    assert!(ext("B2,ds1.select(qty)"), "Comma 两侧或 ⇒ true");
}

/// `ValueList.isExtended() = param == null || param.getType() != ':'`（`ValueList.java:30-32`）。
#[test]
fn value_list_extends_unless_the_top_level_separator_is_colon() {
    assert!(ext("[]"), "空参数 = param null ⇒ true");
    assert!(ext("[1]"), "叶子参数 ⇒ true");
    assert!(ext("[1,2]"), "顶层 `,` ⇒ true");
    assert!(!ext("[1:3]"), "顶层 `:` ⇒ false");
    assert!(
        ext("list(1,2)"),
        "`list(...)` 登记为 ValueList（FunctionLib.java:165）⇒ true"
    );
    assert!(!ext("list(1:3)"));
}

/// 叶节点全为 false：`CSVariable`（`CSVariable.java:379-381`）、`DSVariable`、`UnknownSymbol`。
#[test]
fn leaf_nodes_are_never_extendable() {
    for src in ["B2", "$B2", "AA10", "qty", "true", "12", "1.5", "'text'"] {
        assert!(!ext(src), "叶节点 `{src}` 应为 false");
    }
}

/// 前缀运算符：`Operator.isExtended() = left || right`，前缀只挂 `right`。
#[test]
fn prefix_operators_look_at_their_operand() {
    assert!(!ext("not B2"));
    assert!(!ext("-B2"));
    assert!(ext("not ds1.select(qty)"), "操作数可扩展 ⇒ 前缀也可扩展");
}

// ---------------------------------------------------------------------------
// 优先级 / 树形（§13.1 的绑定力表 —— 等价性论证的落地）
// ---------------------------------------------------------------------------

/// `*`(14) 比 `+`(13) 紧：`1+2*3` ⇒ `1 + (2*3)`。
#[test]
fn multiplication_binds_tighter_than_addition() {
    let e = parse("1+2*3").unwrap();
    let Expr::Binary {
        op: BinOp::Add,
        lhs,
        rhs,
    } = &e
    else {
        panic!("顶层应为 +");
    };
    assert_eq!(**lhs, Expr::Const(Value::Int(1)));
    assert!(
        matches!(**rhs, Expr::Binary { op: BinOp::Mul, .. }),
        "右操作数应为 2*3"
    );
}

/// 全部中缀运算符**左结合**（`Expression.java:442` 的严格 `<`）。
#[test]
fn binary_operators_are_left_associative() {
    let e = parse("1-2-3").unwrap();
    let Expr::Binary {
        op: BinOp::Sub,
        lhs,
        rhs,
    } = &e
    else {
        panic!("顶层应为 -");
    };
    assert!(
        matches!(**lhs, Expr::Binary { op: BinOp::Sub, .. }),
        "应为 (1-2)-3"
    );
    assert_eq!(**rhs, Expr::Const(Value::Int(3)));
    // `:`（4）也是左结合：(B2:B4):B6
    let l = parse("B2:B4:B6").unwrap();
    assert!(matches!(
        l,
        Expr::Binary {
            op: BinOp::Link,
            ..
        }
    ));
}

/// 后缀 `[...]`(17) 与一元 `-`(18)：`-B2[1]` ⇒ `(-B2)[1]`（17 < 18 ⇒ 后缀包住前缀）。
#[test]
fn postfix_index_on_unary_minus_wraps_the_whole_unary() {
    let e = parse("-B2[1]").unwrap();
    let Expr::Index { target, .. } = &e else {
        panic!("顶层应为后缀 [...]");
    };
    assert!(
        matches!(**target, Expr::Unary { op: UnOp::Neg, .. }),
        "应为 (-B2)[1]"
    );
    // `not`(16) < 后缀(17) ⇒ 后缀先绑：not (B2[1])
    let n = parse("not B2[1]").unwrap();
    let Expr::Unary { op: UnOp::Not, rhs } = &n else {
        panic!("顶层应为 not");
    };
    assert!(matches!(**rhs, Expr::Index { .. }), "应为 not (B2[1])");
}

/// 格引用优先于调用（`Expression.java:201-203` 在 `:210-228` 之前）⇒ `B2(1)` 是 `Offset`。
#[test]
fn cell_reference_wins_over_function_call() {
    let e = parse("B2(1)").unwrap();
    assert!(
        matches!(e, Expr::Index { .. }),
        "`B2(1)` 应为 Offset(CSVariable)"
    );
    let f = parse("sum(1,2)").unwrap();
    assert!(matches!(f, Expr::Call(_)), "`sum(1,2)` 应为函数调用");
}

/// `@参数` / `@@宏参数` / `@` 当前格（`Expression.java:178-192`）—— **建树期绑定**（M3-01）。
///
/// 三种形态 × 登记与否的完整真值表（真机证据 RUNLOG §15/§24.1）：
///
/// | 写法 | 已登记 | 未登记 |
/// |------|--------|--------|
/// | `@p`  | `Constant(值)` | **报错**（`:189-191`，真机：`未知的参数：@p`） |
/// | `@@p` | `Constant(值)` | `Constant(null)`，**不报错**（`:183-186` 不查 `isParamName`） |
/// | `@`   | `CSCurrent`（与参数表无关） | 同左 |
#[test]
fn parameter_forms_are_classified() {
    let mut t = ParamTable::new();
    t.insert("p", Value::Int(7));

    assert_eq!(parse_with("@p", &t).unwrap(), Expr::Const(Value::Int(7)));
    assert_eq!(parse_with("@@p", &t).unwrap(), Expr::Const(Value::Int(7)));
    assert_eq!(parse("@").unwrap(), Expr::CurrentCell);

    // 未登记：`@p` 报错、`@@q` 不报错 —— 两条路径的判据本就不同（见上表）
    assert!(
        matches!(
            parse_with("@p", &ParamTable::EMPTY),
            Err(crate::error::EngineError::UnknownParam { .. })
        ),
        "未登记 `@p` 必须报 UnknownParam"
    );
    assert_eq!(
        parse_with("@@q", &ParamTable::EMPTY).unwrap(),
        Expr::Const(Value::Null),
        "未登记 `@@q` 静默取 null（`:183-186` 不查 `isParamName`）"
    );
}

/// 9 个夹具里出现过的**全部 15 种不同公式**（28 处实例；`engine_types` 无表达式格）。
/// 条数与清单由 `tests/fixtures/engine_*.expected.json` 的 `formulas[]` 逐条核对。
#[test]
fn every_formula_appearing_in_the_fixtures_classifies_correctly() {
    // (表达式, 期望 is_extend, 出现于)
    let cases: &[(&str, bool, &str)] = &[
        (
            "ds1.select(region)",
            true,
            "engine_ext/merge/samemerge/pager/expr",
        ),
        (
            "ds1.select(qty)",
            true,
            "engine_ext/merge/samemerge/pager/expr/disp",
        ),
        ("B2", false, "engine_expr/cycle"),
        ("C2", false, "engine_cycle"),
        ("6*1", false, "engine_disp"),
        ("$B2*3", false, "engine_expr"),
        ("B2+$B2", false, "engine_expr"),
        ("B2*2", false, "engine_sum"),
        ("B3*2", false, "engine_pager"),
        ("sum(B2:B4)", false, "engine_sum"),
        ("ds1.sum(qty)", false, "engine_sum"),
        ("sum(ds1.select(qty))", false, "engine_sum"),
        ("ds2.sum(qty)", false, "engine_sum"),
        ("sum(ds2.select(qty))", false, "engine_sum"),
        ("ds2.count()", false, "engine_sum"),
        // M3-02 GBK 夹具：中文列名/中文字面量的三种形态（`@p` 等参数公式
        // 无参不可解析，由 `engine_param_formulas_classify_*` 单独覆盖）。
        (
            "ds1.select(区域)",
            true,
            "engine_gbk（中文列名 DS select 仍可扩展）",
        ),
        (
            "ds1.区域 + \"站\"",
            false,
            "engine_gbk（DSVariable 裸列引用不扩展 + 中文串拼接）",
        ),
    ];
    for (src, want, where_) in cases {
        assert_eq!(ext(src), *want, "`{src}`（{where_}）应为 {want}");
    }
    assert_eq!(cases.len(), 17, "17 种不同公式（夹具全量去重）");
    // 独立核算：3 条为真，与产品实测一致 —— `engine_sum` 行 2 的
    // ds1.select(region)/ds1.select(qty) 被拓扑改写为 18（§4.1），
    // `engine_gbk` 的 ds1.select(区域) 同理（中文列名不改变 Select 的
    // isExtend 判定；裸 ds1.区域 是 DSVariable ⇒ false）。
    let true_count = cases.iter().filter(|(_, w, _)| *w).count();
    assert_eq!(true_count, 3, "夹具里只有三条公式可扩展");
}

/// `engine_param`（M3-01）的 4 种公式 —— 必须**带参数表**才能解析（`@名` 未登记
/// 直接报错，不再是"保守取 false"）。四条各钉一条规则：
///
/// | 公式 | is_extend | 钉住的规则 |
/// |------|-----------|-----------|
/// | `@p`（List[1,2,3]） | **true** | 参数值 `instanceof List` ⇒ 驱动扩展（真机：矩阵 3→5 行） |
/// | `sum(@p)` | false | 聚合**不透传** is_extend（`Function.java` 未覆写）—— 操作数为真也白搭 |
/// | `max(@p)` | false | 同上（max/min/sum 三条同规则） |
/// | `@s`（Integer(7)） | false | 标量参数 ⇒ 普通公式格（17），矩阵不变 |
#[test]
fn engine_param_formulas_classify_with_their_injected_values() {
    /// (公式, 期望 is_extend, 注入参数)。用具名别名避免 `clippy::type_complexity`
    /// —— 四层嵌套的裸元组类型会被 `-D warnings` 拦（`pager_guard.rs` 同款先例）。
    type ParamCase<'a> = (&'a str, bool, &'a [(&'a str, Value)]);
    let list3: Value = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    let cases: &[ParamCase] = &[
        ("@p", true, &[("p", list3.clone())]),
        ("sum(@p)", false, &[("p", list3.clone())]),
        ("max(@p)", false, &[("p", list3.clone())]),
        ("@s", false, &[("s", Value::Int(7))]),
    ];
    for (src, want, params) in cases {
        assert_eq!(
            ext_with(src, params),
            *want,
            "`{src}`（engine_param，注入 {params:?}）应为 {want}"
        );
    }
    // 未登记的 `@p` 走**显式报错**而不是"保守 false"（产品 `Expression.java:189-191`）。
    assert!(
        parse_with("@p", &ParamTable::EMPTY).is_err(),
        "engine_param 的公式在无参表下必须报错 —— 静默 false 会掩盖漏注入"
    );
}

// ---------------------------------------------------------------------------
// 边界：错误出口必须**结构化**，绝不 panic
// ---------------------------------------------------------------------------

/// 语法错误返回 `Err`，不是 panic（产品同样抛 `ReportError`，`Expression.java:489-493`）。
#[test]
fn malformed_input_returns_a_structured_error() {
    for bad in ["1 +", "(1", "1)", "B2[", "'unterminated", "1 | 2", "and"] {
        assert!(parse(bad).is_err(), "`{bad}` 应报错而不是 panic");
    }
}

/// 一步到位入口与 `parse(..).is_extend()` 一致。
#[test]
fn is_extend_convenience_matches_parse_then_predicate() {
    for src in ["ds1.select(q)", "B2*2", "sum(ds1.select(q))", "[1:2]"] {
        assert_eq!(
            is_extend(src, &ParamTable::EMPTY).unwrap(),
            parse(src).unwrap().is_extend()
        );
    }
}

// `is_ds_function()` / `get_state()` 的真值表与前提钉子 —— 自本文件拆出（门禁行数），
// 出处与"为什么两条链必须分开"见 `super::dsstate` 的模块文档。
mod dsstate;

// 与产品源码的 4 处分叉 + 函数优先级表，各带一条回归；见 `tests::deviation`。
mod deviation;
