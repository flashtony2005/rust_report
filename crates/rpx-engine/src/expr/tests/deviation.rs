//! 与产品源码的**四处分叉**与函数优先级表的回归测试。
//!
//! 每条都挂着 `file:line`。这四条是 2026-10 由架构师提出、逐条回反编译源码复核后
//! 定案的：复核结论见各条注释。**零夹具覆盖的条目一律标「未验证」。**

use super::*;
use crate::params::ParamTable;
use crate::value::Value;

/// ✅ **分叉 1（真机已验证）**：`@p` / `@@p` 在产品里是 `Constant(ctx.getParamValue(id))`
/// （`Expression.java:185` 的 `@@`、`:188` 的 `@`）与 `ParamNode`（`:233`），
/// 三者都继承 `Constant.isExtended() = value instanceof List`（`Constant.java:22-24`）。
///
/// 真机证据（`tests/fixtures/probe_param_list_out.utf8.txt`）：
///
/// | 参数值 | `@p` | `@@p` | 全量 calc 的后果 |
/// |--------|------|-------|-----------------|
/// | `Integer(7)` / `String("x")` | false | false | 格不变（3×2） |
/// | `List[1,2,3]` | **true** | **true** | **矩阵长到 5×2，锚点 extend=18 subCells=3** |
/// | `List[9]`（单元素） | **true** | **true** | extend 改写成 18 但 subCells=1 |
/// | `List[]`（空） | **true** | **true** | ⚠ 成为扩展格却没有子格 |
///
/// ⇒ 判据是 `instanceof List`、**不看内容**，所以单元素与空 List 都为真。
///
/// M3-01 起这条走**完整链路**（参数表 → `parse_with` 建树期内联 → `is_extend`），
/// 覆盖的是"绑定值真的进了 AST"而不只是"某个枚举分支持续返回真"。
#[test]
fn param_bound_to_a_list_value_is_extendable() {
    let extend_of = |v: Value| {
        let mut t = ParamTable::new();
        t.insert("p", v);
        parse_with("@p", &t)
            .unwrap_or_else(|e| panic!("`@p` 已登记却解析失败：{e}"))
            .is_extend()
    };
    // 三个 List 形态全部为真（含空 —— 判据是 instanceof，不看长度）
    for v in [
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
        Value::List(vec![Value::Int(9)]),
        Value::List(vec![]),
    ] {
        assert!(
            extend_of(v.clone()),
            "`@p` 绑 {v:?} 必须为 true（真机验证）"
        );
    }
    // 标量参数为假
    for v in [Value::Int(7), Value::Text("x".into())] {
        assert!(
            !extend_of(v.clone()),
            "`@p` 绑 {v:?} 必须为 false（真机验证）"
        );
    }
    // ⛔ M3-01 起"未登记"的语义是**建树期报错**，不再是"保守取 false"——
    //   产品 `:189-191` 抛 ReportError（真机消息见 RUNLOG §24.1）。静默取 false 会把
    //   该扩展的格判成不扩展 ⇒ 会算错；显式失败才安全。
    assert!(
        matches!(
            parse_with("@p", &ParamTable::EMPTY),
            Err(crate::error::EngineError::UnknownParam { .. })
        ),
        "未登记 `@p` 必须报 UnknownParam"
    );
}

/// ⛔ **值层**：单元素 / 空 List 下 `Expr` 的最终形态（team-lead 2026-10 要求补的两条）
///
/// 谓词层（`is_extend()`）已由 `param_bound_to_a_list_value_is_extendable` 覆盖；
/// 这里覆盖**值层**：这两种形态必须**原样保留参数值**并**照样判真**，而且
/// 「extend 被改写成 18」与「subCells 有几个」是**两条独立路径**，不能互相推导。
///
/// 真机依据（`tests/fixtures/probe_param_list_out.utf8.txt`）：
///
/// | 参数值 | isExtend | 全量 calc |
/// |--------|----------|-----------|
/// | `List[9]`（单元素） | true | `extend` 改写成 **18**，`subCells=1` |
/// | `List[]`（空） | true | 同样 18 + `subCells=1`；且 `ExtExtendCell.java:376-384` |
/// | | | 对空 List 提前 `setValue(null)` 并 return |
///
/// ⇒ **不显式处理就会写成错的两种**：单元素 ⇒「当标量、不改写」⇒ 少一次扩展、
/// 格被错判 PlainFormula(17)；空 List ⇒「不算 List」⇒ 该格的值与类型都不同。
#[test]
fn single_element_and_empty_list_params_keep_their_value_and_still_extend() {
    let bound = |v: Value| {
        let mut t = ParamTable::new();
        t.insert("p", v);
        parse_with("@p", &t).unwrap_or_else(|e| panic!("`@p` 已登记却解析失败：{e}"))
    };

    // ---- 单元素 ----
    let one = Value::List(vec![Value::Int(9)]);
    let e1 = bound(one.clone());
    assert!(
        e1.is_extend(),
        "单元素 List 仍判真（真机：extend 改写成 18、subCells=1）"
    );
    // 值层：建树期内联为 Const，值原样保留，求值期能直接取到
    assert_eq!(e1, Expr::Const(one.clone()), "单元素 List 的值必须原样内联");

    // ---- 空 List ----
    let empty = Value::List(vec![]);
    let e2 = bound(empty.clone());
    assert!(
        e2.is_extend(),
        "空 List 也判真（判据是 instanceof，不看长度）"
    );
    assert_eq!(
        e2,
        Expr::Const(empty.clone()),
        "空 List 的值必须原样内联（长度 0，不是缺值）"
    );

    // ---- 两条路径独立性：真机上「extend=18」与「subCells=1」同时成立 ----
    // 本切片不产出 sub_cells（那是扩展算法的产物），但要显式记下这条约束：
    // 实现 M2-15 时**不能**用 subCells 数量反推是否改写，也不能用 extend 反推子格数。
    assert_eq!(
        (e1.is_extend(), one_list_len(&e1)),
        (true, 1),
        "改写判定与子格数是两个量：extend 会被改写成 18，而 subCells 仍是 1"
    );
    assert_eq!(
        (e2.is_extend(), one_list_len(&e2)),
        (true, 0),
        "空 List：extend 同样改写成 18，subCells 仍是 1（不是 0）"
    );
}

/// 取内联常量里 List 的长度；非 List 记 -1。只给上面那条测试用。
fn one_list_len(e: &Expr) -> i32 {
    match e {
        Expr::Const(Value::List(items)) => items.len() as i32,
        _ => -1,
    }
}

/// ⛔ **分叉 2**：`Select` 的 `left`（= 本切片的 `target`）**不是**花括号里的表达式。
/// `Expression.java:131-134` 只调 `setParameter(...)`；`SpecialFunction.isExtended()`
/// （`:27-32`）读的是 `left`。前缀位置（表达式开头）没有前驱 ⇒ `target = None`
/// ⇒ **恒 false**，尽管参数里的 `DSSelect` 是 true。
///
/// ⚠ M7-8 起 `target` 被**建模**了（后缀 `Cellx{...}` 会挂上前驱），于是
/// 「恒 false」这条结论**收窄为**：`target == None` 时为 false；`target` 存在时
/// 取 `target.is_extend()`（与源码 `left.isExtended()` 同构）。`C2{}` 仍是 false
/// —— `CSVariable.isExtended()` 恒 false（`CSVariable.java:379-381`）。
#[test]
fn select_brace_is_never_extendable_even_when_its_argument_is() {
    let e = parse("{ds1.select(qty)}").unwrap();
    let Expr::Select { target, arg } = &e else {
        panic!("顶层应为 Select");
    };
    assert!(target.is_none(), "前缀花括号没有前驱 ⇒ target = None");
    let Expr::Call(inner) = &**arg else {
        panic!("参数应是 DSSelect 调用");
    };
    assert_eq!(inner.kind, CallKind::DsExtend, "参数本身是可扩展的");
    assert!(
        !e.is_extend(),
        "⛔ 但 Select 读 left（此处 null）⇒ 整条为 false"
    );
    // 同形对照：参数为 ValueList（isExtended=true）也照样 false
    assert!(!ext("{[1,2]}"));
    // 后缀形态：`C2{}` 的前驱是 CSVariable（恒 false）⇒ 仍 false
    assert!(!ext("B2{}"), "CSVariable 前驱 ⇒ 仍 false");
    // 后缀形态：前驱本身可扩展 ⇒ 透传为 true（原先错判 false 的一条）
    assert!(ext("ds1.select(qty){}"), "DSSelect 前驱 ⇒ true");
}

/// ⛔ **分叉 3**：`$[...]` 是**字符串常量**，不是格引用（`Expression.java:169-172`
/// 的 `new Constant(Escape.remove(内文.trim()))`）。
#[test]
fn dollar_bracket_forms_a_string_constant_not_a_cell_reference() {
    for (src, want) in [("$[abc]", "abc"), ("$[ a b ]", "a b"), ("$[a\\nb]", "a\nb")] {
        let got = parse(src).unwrap();
        assert_eq!(
            got,
            Expr::Const(Value::Text(want.to_string())),
            "`{src}` 应为常量"
        );
    }
    assert!(!ext("$[1,2]"), "字符串常量不是扩展源（`Value::Text`）");
}

/// ⛔ **分叉 4**：产品的格号判定是 `((ExtCellSet)cs).getSource(id) != null`
/// （`Expression.java:201`），**不是**形态判定。本切片没有引擎矩阵，只能暂定 ——
/// 所以必须打 `provisional` 标记，让求值期无法误以为它已经是 `CSVariable`。
#[test]
fn cell_shape_check_is_marked_provisional() {
    // 形态像格号的数据集列名会被暂定成 CellRef —— 这正是要标记出来的坑
    for id in ["qty1", "amount2", "B2"] {
        let e = parse(id).unwrap();
        let Expr::CellRef { provisional, .. } = e else {
            panic!("`{id}` 应暂定为 CellRef");
        };
        assert!(provisional, "`{id}` 的 CellRef 必须标 provisional=true");
    }
    // `provisional` 不影响 is_extend：CSVariable 与 UnknownSymbol 都是 false
    assert!(
        !ext("qty1"),
        "误判不改变 is_extend —— 风险只在求值期（M2-06）"
    );
}

/// ✅ **转义规则 = `Escape.remove`（`Escape.java:173-214`，逐行核对）**。
///
/// 这条**不影响 `is_extend()`**（谓词只看树形不看字面量），但影响 M2-13 的求值，
/// 所以必须**逐条**对齐而不是"最小实现"。
#[test]
fn string_escapes_follow_escape_remove_exactly() {
    use crate::expr::lexer::unescape;
    // :191-202 三个具名转义 → 9 / 13 / 10
    assert_eq!(unescape(r"a\tb"), "a\tb");
    assert_eq!(unescape(r"a\rb"), "a\rb");
    assert_eq!(unescape(r"a\nb"), "a\nb");
    // :203-206 default：未知转义**只留后一个字符**，反斜杠被丢掉
    assert_eq!(unescape(r"a\xb"), "axb", "未知转义 \\x 只留 x");
    assert_eq!(unescape(r"a\\b"), r"a\b", "\\\\ 留一个反斜杠");
    assert_eq!(unescape(r#"a\"b"#), "a\"b", "\\\" 留引号");
    // :208-210 非 escape 字符原样；空串原样（:178-180）
    assert_eq!(unescape("abc"), "abc");
    assert_eq!(unescape(""), "");
    // ⛔ :188 **结尾的孤立反斜杠被丢弃** —— 旧实现写的是"保留反斜杠"，与产品不符。
    // ⚠ 这条是**源码依据、真机不可达**：产品两条入口都构造不出能走到这里的样本 ——
    //   字面量入口会因为引号不闭合而报错；`$[...]` 入口见下面那条（产品报"未知的单元格：$"）。
    //   实现仍按源码逐行对齐（比"猜一个行为"更保守），但**不宣称已真机验证**。
    assert_eq!(
        unescape(r"abc\"),
        "abc",
        "尾部孤立反斜杠：产品丢弃（Escape.java:188）"
    );
    assert_eq!(unescape(r"\"), "", "只有一个孤立反斜杠 ⇒ 空串");
}

/// `$[...]` 的字面内容也走同一个 `Escape.remove`（`Expression.java:171`），
/// ⛔ 而且**先 `trim()` 再反转义**（顺序不可交换）。
#[test]
fn dollar_bracket_literal_goes_through_the_same_escape_remove() {
    use crate::value::Value;
    for (src, want) in [("$[abc]", "abc"), ("$[ a b ]", "a b"), ("$[a\\nb]", "a\nb")] {
        assert_eq!(
            parse(src).unwrap(),
            Expr::Const(Value::Text(want.to_string())),
            "`{src}` 应为字符串常量"
        );
    }
    // ⛔⛔ **实测发现的差异（不是转义规则，是括号配对）**：`$[a\]` 在**产品上报错**
    //   ——「未知的单元格：$」（真机：probe_escape_out.utf8.txt 的 B 组最后一行）。
    //   原因：产品的 `Sentence.scanBracket` 不把 `\]` 当转义 ⇒ 找不到配对的 `]` ⇒
    //   `$[...]` 分支不成立，落到 `$` 的格引用路径 ⇒ 报"未知的单元格"。
    //   而我们的 `take_group` 会配到 `]` ⇒ 返回常量 `a`。
    //   ⇒ **我们与产品在这里不一致**，已登记为缺口（RUNLOG §18.3），**不假装正确**。
    assert_eq!(
        parse(r"$[a\]").unwrap(),
        Expr::Const(Value::Text("a".to_string())),
        r"当前行为：返回常量 `a`；产品是 ReportError。这是**已知差异**，不是正确行为"
    );
}

// 接线绊线在独立文件里：子模块的 `//!` 模块文档必须紧贴它自己文件的开头。
mod wiring_tripwire;
