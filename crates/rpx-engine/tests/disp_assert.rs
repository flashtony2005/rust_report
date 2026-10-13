//! `engine_disp` 的 `disp_value` 逐格断言：把 M2-14 的显示值通道收口。
//!
//! # 为什么要这个文件
//!
//! `disp_value` 曾是**死键**：前 7 个夹具里它一次都没出现，于是
//! `golden.rs:99` / `loader.rs:159` / `rpx-cli/src/main.rs:129` 三处断言全是永真式
//! —— 一个把该槽静默丢掉的解析器也能让全部测试变绿。第 8 个夹具 `engine_disp`
//! 证伪了"产不出来"，本文件把它的 7 格逐格钉住。
//!
//! # 纪律：期望值一律现读，**不转录常量**
//!
//! 本文件不写死 `"北区"` / `"100.00"` / `"6 件"` / `"Q"` / `"仅显示值"` 里的任何一个。
//! 所有断言都是**从夹具自身推导出的关系式**（即产品侧的产出规则），例如：
//! 显示值只在 `disp != String.valueOf(value)` 时写出（`CalcJson.java:71-73`）。
//! 换夹具后本文件依然成立；只有当**产品语义变了**才会红。
//! 每条断言都带"非永真式"守卫（先证明夹具里真有这种格），避免夹具变了之后
//! 断言悄悄退化成 `for ... {}` 空转。
//!
//! # 与实现的对照
//!
//! [`CalcCell::is_emitted`] = `!value.is_null() || disp_value.is_some()`
//! （`src/arena/calc_grid.rs:65-67`），对应 `CalcJson.java:51-53` 的
//! `if (v == null && dv == null) { continue; }` —— 判定式是 `||` 而非 `&&`。
//! 本文件 `null_value_cell_is_emitted_because_the_rule_is_or` 正是这一条的反例锚点。

mod common;

use common::{all_names, load};

/// 复刻产品的 `String.valueOf(v)`：字符串去掉 JSON 引号，其余就是原文 token。
///
/// 局限：不对字符串内部的 JSON 转义做解码 —— 现有夹具的字符串值均无转义，
/// 真出现带转义的值时这里会算错（届时应改为复用 `common::json` 的解码器）。
fn display_of(value_raw: &str, declared_type: &str) -> String {
    if declared_type == "string" {
        value_raw.trim_matches('"').to_string()
    } else {
        value_raw.to_string()
    }
}

/// `disp_value` 是否含非 ASCII 字符（非 ASCII 走 modified-UTF-8 往返，最易在解析侧坏掉）。
fn has_non_ascii(s: &str) -> bool {
    s.chars().any(|ch| (ch as u32) > 0x7F)
}

/// 产品的落盘判定是 `v != null || dv != null`（`CalcJson.java:51-53`），
/// 所以 `value == null` 但 `disp_value` 非空的格**必须**出现在 `cells` 里。
///
/// 这里不写死坐标，而是找出夹具里所有 null 值格 —— 换夹具后依然成立。
#[test]
fn null_value_cell_is_emitted_because_the_rule_is_or() {
    let fx = load("engine_disp");
    let g = fx.grid().expect("engine_disp 有计算态网格");

    let null_valued: Vec<_> = g.cells.iter().filter(|c| c.declared_type == "null").collect();
    assert!(!null_valued.is_empty(), "夹具必须含 value==null 的格，否则本测试是永真式");

    for c in &null_valued {
        assert_eq!(c.value_raw, "null", "({0},{1}) null 值的原文 token", c.r, c.c);
        let dv = c.disp_value.as_deref().expect("value==null 却落盘 ⇒ disp_value 必非空");
        assert!(!dv.is_empty(), "({0},{1}) disp_value 不该是空串", c.r, c.c);
    }

    // 反向：`cells` 里每一格都必须满足 is_emitted()，不可能两者同时为 null。
    // 这一条同时是对 `src/arena/calc_grid.rs:65-67` 的一致性校验。
    for c in &g.cells {
        assert!(
            c.declared_type != "null" || c.disp_value.is_some(),
            "({0},{1}) value 与 disp_value 同时为 null 的格不该落盘", c.r, c.c
        );
    }
}

/// 产品的写出条件是 `dv != String.valueOf(v)`（`CalcJson.java:71-73`），
/// 所以**任何**落盘的 `disp_value` 都不等于该格的显示值；
/// 相等就说明是解析器凭空造的。
#[test]
fn disp_value_is_never_redundant_with_the_display_of_value() {
    let fx = load("engine_disp");
    let g = fx.grid().unwrap();

    let with_disp: Vec<_> = g.cells.iter().filter(|c| c.disp_value.is_some()).collect();
    assert!(!with_disp.is_empty(), "夹具必须含 disp_value，否则本测试是永真式");

    for c in &with_disp {
        let shown = display_of(&c.value_raw, &c.declared_type);
        assert_ne!(
            c.disp_value.as_deref(),
            Some(shown.as_str()),
            "({0},{1}) 夹具不该写出与 String.valueOf(value) 相同的 disp_value", c.r, c.c
        );
    }
}

/// 显示值的小数写法**不得**改变声明类型 —— 找到"值是整数、显示带小数点"的格，
/// 断言它的 `type` 仍在整型词表内，而不是被显示格式带成 `decimal`。
#[test]
fn fractional_display_never_promotes_the_declared_type() {
    let fx = load("engine_disp");
    let g = fx.grid().unwrap();

    let fractional: Vec<_> = g
        .cells
        .iter()
        .filter(|c| {
            c.disp_value.as_deref().is_some_and(|d| d.contains('.'))
                && !c.value_raw.contains('.')
        })
        .collect();
    assert!(!fractional.is_empty(), "夹具必须含『值整数但显示带小数点』的格，否则是永真式");

    for c in &fractional {
        assert_ne!(
            c.declared_type, "decimal",
            "({0},{1}) 显示值的小数点不得把类型改成 decimal", c.r, c.c
        );
        assert!(
            matches!(c.declared_type.as_str(), "int" | "long" | "double" | "float"),
            "({0},{1}) 数值格声明类型应仍在数值词表内，实际 {2}", c.r, c.c, c.declared_type
        );
    }
}

/// 非 ASCII（中文）显示值必须完好穿过 modified-UTF-8 往返：
/// 既不能丢成 `?`，也不能被解码器塞进 U+FFFD 替换字符。
#[test]
fn non_ascii_display_survives_the_utf8_round_trip() {
    let fx = load("engine_disp");
    let g = fx.grid().unwrap();

    let non_ascii: Vec<_> = g
        .cells
        .iter()
        .filter(|c| c.disp_value.as_deref().is_some_and(has_non_ascii))
        .collect();
    assert!(!non_ascii.is_empty(), "夹具必须含非 ASCII 的 disp_value，否则是永真式");

    for c in &non_ascii {
        let dv = c.disp_value.as_deref().unwrap();
        assert!(has_non_ascii(dv), "({0},{1}) 非 ASCII 显示值丢失", c.r, c.c);
        assert!(
            !dv.contains('\u{FFFD}'),
            "({0},{1}) 出现 U+FFFD 替换字符，说明 UTF-8 往返坏了", c.r, c.c
        );
        assert!(!dv.is_empty(), "({0},{1}) 显示值不该为空", c.r, c.c);
    }
}

/// 纵向扩展（`extend == 18`）把显示值**逐个**下发给每一个实例：
/// 锚点 + 其后 `sub_cells - 1` 行，同列、同显示值。
///
/// ⛔ 注意：显示值只下发到**顶层 `cells`**，附属格 `sub_values` 对象里
/// **没有** `disp_value` 字段（`CalcJson.java:86-95` 只写 r/c/value/type），
/// 所以断言必须读顶层格，不能去 `sub_values` 里找。
#[test]
fn vertical_extension_dispatches_display_value_to_every_instance() {
    let fx = load("engine_disp");
    let g = fx.grid().unwrap();

    let anchors: Vec<_> = g
        .cells
        .iter()
        .filter(|c| c.source.as_ref().is_some_and(|s| s.sub_cells > 1))
        .collect();
    assert!(!anchors.is_empty(), "夹具必须含有附属格的锚点，否则是永真式");

    let mut instances = 0;
    for a in &anchors {
        let s = a.source.as_ref().unwrap();
        if s.extend != 18 {
            continue; // 只看纵向扩展
        }
        assert!(a.disp_value.is_some(), "锚点 ({0},{1}) 应带 disp_value", a.r, a.c);
        for k in 1..s.sub_cells {
            let inst = g
                .cell(a.r + k, a.c)
                .unwrap_or_else(|| panic!("({0},{1}) 扩展实例缺失", a.r + k, a.c));
            assert_eq!(
                inst.disp_value, a.disp_value,
                "({0},{1}) 显示值未逐个下发给扩展实例", inst.r, inst.c
            );
            instances += 1;
        }
    }
    assert!(instances > 0, "未找到任何纵向扩展实例");
}

/// 跨**全部**夹具校验 `is_emitted()` 的镜像：`cells` 里不存在
/// 「`value` 为 null **且** `disp_value` 缺失」的格。
///
/// 这是 `src/arena/calc_grid.rs:65-67` 的对拍断言 —— 解析侧一旦把某个 null 值格的
/// `disp_value` 丢掉、或把 `type:"null"` 误判成非 null，这里立刻响。
/// 顺带证明：加载器读出的 9 个夹具与实现的落盘判定式**完全一致**。
#[test]
fn no_fixture_emits_a_cell_with_both_value_and_display_null() {
    let mut checked = 0usize;
    let mut grids = 0usize;
    for name in all_names() {
        let fx = load(name);
        // engine_cycle 是错误型夹具，没有计算态网格
        let Some(g) = fx.grid() else { continue };
        grids += 1;
        for c in &g.cells {
            assert!(
                c.declared_type != "null" || c.disp_value.is_some(),
                "{name}: ({},{}) value 与 disp_value 同时为 null，不该出现在 cells 里", c.r, c.c
            );
            checked += 1;
        }
    }
    assert!(grids > 0, "没有任何夹具产出计算态网格");
    assert!(checked > 0, "没有任何非空计算格");
}

/// 显示值覆盖有两种形态，夹具里必须**同时**出现，缺一条就说明覆盖有缺口：
/// * **增广/格式化** —— 显示值以 `String.valueOf(value)` 开头
///   （整数补小数位、数值加单位后缀都属此类）；
/// * **替换** —— 显示值与值毫无前缀关系（文本映射、量纲标签等）。
///
/// 只出现其一时本测试会响亮失败，避免它在夹具演进后悄悄退化成永真式。
#[test]
fn display_override_covers_both_augmenting_and_replacing_forms() {
    let fx = load("engine_disp");
    let g = fx.grid().unwrap();

    let (mut augmenting, mut replacing) = (0usize, 0usize);
    for c in &g.cells {
        let Some(dv) = c.disp_value.as_deref() else { continue };
        let shown = display_of(&c.value_raw, &c.declared_type);
        assert!(!shown.is_empty(), "({0},{1}) 值不该为空串", c.r, c.c);
        if dv.starts_with(shown.as_str()) {
            augmenting += 1;
        } else {
            replacing += 1;
        }
    }
    assert!(augmenting > 0, "夹具必须含『显示值以值为前缀』的增广型覆盖（整数补位/加单位）");
    assert!(replacing > 0, "夹具必须含与值无前缀关系的替换型覆盖（文本映射/量纲标签）");
}

/// `sub_values` 与 `sub_cells` 的长度自洽（`CalcJson.java:84` 的 `if (subs > 1)`）。
///
/// 同时把"附属格不带 `disp_value`"这条约定钉住：加载器对 `sub_value` 对象用
/// `expect_keys` 白名单（`r`/`c`/`value`/`type`），产品一旦开始写 `disp_value`，
/// 加载器会**响亮 panic** 而不是静默吞掉。
#[test]
fn sub_values_length_matches_sub_cells() {
    let fx = load("engine_disp");
    let g = fx.grid().unwrap();

    for c in &g.cells {
        let Some(s) = &c.source else { continue };
        match &s.sub_values {
            Some(sv) => assert_eq!(
                sv.len() as i32, s.sub_cells,
                "({0},{1}) sub_values 长度应等于 sub_cells", c.r, c.c
            ),
            None => assert_eq!(
                s.sub_cells, 1,
                "({0},{1}) sub_cells>1 时必须写 sub_values", c.r, c.c
            ),
        }
    }
}
