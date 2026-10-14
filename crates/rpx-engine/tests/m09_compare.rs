//! M2-09 对拍器的**空跑自测**与三类差异的定向验证。
//!
//! # 本轮做的是什么、没做什么
//!
//! 引擎侧的导出入口还没定，所以本文件**不**做「引擎 vs 夹具」的真实对拍，
//! 只做两件能立刻做完、且能证明对拍器本身没写错的事：
//!
//! 1. **空跑自测**：拿每个夹具与它自己比 ⇒ 必须**零差异**。
//! 2. **定向变异**：把夹具 JSON 改一处（删一格 / 改一值 / 加一格），
//!    验证对拍器**分别**报出 `missing` / `mismatch` / `extra` —— 纪律 ① 要求三类都能报。
//!
//! 纪律 ②（稀疏格不算缺失）与 ③（`source` 可选、`calc≠src`）也在这里被显式钉住。
//!
//! 变异全部在**内存里的字符串**上做（读夹具原文 → 改 → 传给对拍器），
//! **不碰磁盘上的夹具文件**。

mod common;

use common::{all_names, compare_actual_text, compare_fixture_to_itself, Kind};

/// 在 `calculated.cells` 数组里**删掉第 `idx` 个格对象**（按大括号配平，跳过字符串内的括号）。
///
/// 不用硬编码整段格文本：夹具的格结构会随产品演进（已经历过一次加键），
/// 硬编码会让这条测试因为"无关的 schema 变化"而红。
fn delete_nth_cell(text: &str, idx: usize) -> String {
    let at = text.find("\"cells\":[").expect("夹具应有 calculated.cells") + "\"cells\":[".len();
    let b = text.as_bytes();
    let mut i = at;
    let mut seen = 0usize;
    while i < b.len() {
        if b[i] != b'{' {
            i += 1;
            continue;
        }
        // 从这个 `{` 起配平，得出该格对象的结束位置
        let start = i;
        let (mut depth, mut in_str, mut esc) = (0i32, false, false);
        while i < b.len() {
            match b[i] {
                b'"' if !esc => in_str = !in_str,
                b'\\' if in_str => esc = !esc,
                b'{' if !in_str => depth += 1,
                b'}' if !in_str => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            if b[i] != b'\\' {
                esc = false;
            }
            i += 1;
        }
        if seen == idx {
            // 连同前置逗号一起删，避免留下 `[,`
            let mut s = start;
            if s > at && b[s - 1] == b',' {
                s -= 1;
            }
            let mut e = i + 1;
            if e < b.len() && b[e] == b',' {
                e += 1;
            }
            let mut out = String::with_capacity(text.len());
            out.push_str(&text[..s]);
            out.push_str(&text[e..]);
            return out;
        }
        seen += 1;
        i += 1;
    }
    panic!("cells 里没有第 {idx} 个格");
}

/// 读夹具的原始 JSON 文本（供内存变异用）。
fn fixture_text(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(format!("{name}.expected.json"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("读取 {} 失败：{e}", path.display()))
}

/// ① 空跑自测：15 个夹具与自身对比必须零差异。
///
/// 这一条同时证明"零差异"**不是恒真式** —— 若比对逻辑把稀疏格全报成 missing、
/// 或漏比某个维度，这里立刻红。
#[test]
fn self_comparison_is_clean_for_every_fixture() {
    for name in all_names() {
        let r = compare_fixture_to_itself(name);
        assert!(
            r.is_clean(),
            "夹具 {name} 与自身对比竟有差异：\n{}",
            r.summary()
        );
        assert_eq!(r.fixture, name, "报告应记录夹具名");
        assert_eq!(r.expected_dims, r.actual_dims, "{name}: 自比对尺寸应一致");
        // 错误形态夹具（engine_cycle）也要能自比对通过，且没有网格尺寸
        if common::load(name).is_error() {
            assert!(r.expected_dims.is_none(), "{name}: 错误形态不应有网格尺寸");
        } else {
            assert!(r.expected_dims.is_some(), "{name}: 网格形态应有尺寸");
        }
    }
}

/// ② 纪律 ②：`cells` 稀疏**不得**被报成 missing。
///
/// `engine_sum` 的计算态是 8×9 = 72 个位置，但只有 36 个格。
/// 若对拍器遍历 `rows × cols` 全位置，这里会报出 36 条假 missing。
#[test]
fn sparse_cells_do_not_produce_false_missing() {
    let r = compare_fixture_to_itself("engine_sum");
    assert!(r.is_clean());
    let g = common::load("engine_sum");
    let grid = g.grid().unwrap();
    let positions = (grid.rows * grid.cols) as usize;
    assert!(
        positions > grid.cells.len(),
        "本测试的前提是网格位置数 > 实际格数（{} > {}）",
        positions,
        grid.cells.len()
    );
    assert_eq!(
        r.diffs.len(),
        0,
        "稀疏网格被误报出 {} 条差异",
        r.diffs.len()
    );
}

/// ③ 定向变异：删掉一格 ⇒ 必须报 `missing`（纪律 ① 的第一类）。
#[test]
fn a_deleted_cell_is_reported_as_missing() {
    let name = "engine_types";
    let text = fixture_text(name);
    // 删掉 calculated.cells 里的第一格（连它的 source 一起删）
    let mutated = delete_nth_cell(&text, 0);
    assert_ne!(mutated, text, "变异未生效");
    assert!(
        !mutated.contains(r#""r":1,"c":1,"value":"int""#),
        "第 1 格应已被删掉"
    );

    let r = compare_actual_text(&mutated, name);
    assert!(!r.is_clean());
    let (miss, _, _) = r.counts();
    assert!(miss > 0, "删了一格却没报 missing：\n{}", r.summary());
    // 首错必须直接指路
    let first = r.first_error().expect("应能指出第一处错误");
    assert_eq!((first.r, first.c), (1, 1), "首错坐标应是 (1,1)");
    assert_eq!(r.fixture, name, "报告应指明是哪个夹具");
    // 纪律 ⑤：首错要能直接指路，不能只给计数
    let text = r.summary();
    assert!(text.contains("(1,1)"), "摘要应含出错坐标：{text}");
}

/// ④ 定向变异：改一个值 ⇒ 必须报 `mismatch`（不是 missing/extra）。
#[test]
fn a_changed_value_is_reported_as_mismatch() {
    let name = "engine_types";
    let text = fixture_text(name);
    // (2,1) 的 100（int）→ 999
    let mutated = text.replacen(
        r#"{"r":2,"c":1,"value":100,"type":"int""#,
        r#"{"r":2,"c":1,"value":999,"type":"int""#,
        1,
    );
    assert_ne!(mutated, text, "变异未生效");

    let r = compare_actual_text(&mutated, name);
    let (_, _, mis) = r.counts();
    assert!(mis > 0, "改了值却没报 mismatch：\n{}", r.summary());
    assert_eq!(r.counts().0, 0, "改值不应产生 missing");
    assert_eq!(r.counts().1, 0, "改值不应产生 extra");
    let first = r.first_error().unwrap();
    assert_eq!((first.r, first.c), (2, 1));
}

/// ⑤ 定向变异：`Integer(100)` 改成 `Double(100.0)` 的写法 ⇒ 仍算 mismatch
/// （证明对拍器按**原文 token** 比、不做数值归一化，M2-14 的区分不会被抹平）。
#[test]
fn int_and_double_of_the_same_number_still_differ() {
    let name = "engine_types";
    let text = fixture_text(name);
    // (2,1) int 100 -> double 100.0（JSON 里必须带小数点才是 double 形态）
    let mutated = text.replacen(
        r#"{"r":2,"c":1,"value":100,"type":"int""#,
        r#"{"r":2,"c":1,"value":100.0,"type":"double""#,
        1,
    );
    assert_ne!(mutated, text, "变异未生效");
    let r = compare_actual_text(&mutated, name);
    let (_, _, mis) = r.counts();
    assert!(mis > 0, "int→double 未被识别为差异：\n{}", r.summary());
    // 值与类型两条差异都应报出
    let dims: Vec<&str> = r.diffs.iter().map(|d| d.dim.as_str()).collect();
    assert!(dims.contains(&"value"), "应报 value 差异：{dims:?}");
    assert!(dims.contains(&"type"), "应报 type 差异：{dims:?}");
}

/// ⑥ 定向变异：多出一格 ⇒ 必须报 `extra`（纪律 ① 的第三类）。
///
/// 用 `engine_types` 的 (1,1) 格复制一份挂到 (1,9)（模板 4×4 之外，模拟"多算了行/列"）。
#[test]
fn an_extra_cell_is_reported_as_extra() {
    let name = "engine_types";
    let text = fixture_text(name);
    let dup = r#"{"r":9,"c":9,"value":"int","type":"string"}"#;
    let anchor = r#""cells":[{"r":1,"c":1,"value":"int""#;
    assert!(text.contains(anchor), "锚点未找到");
    let mutated = text.replacen(
        anchor,
        &format!(r#""cells":[{dup},{{"r":1,"c":1,"value":"int""#),
        1,
    );
    let r = compare_actual_text(&mutated, name);
    let (_, extra, _) = r.counts();
    assert!(extra > 0, "多出一格却没报 extra：\n{}", r.summary());
    let first = r.first_error().unwrap();
    assert_eq!(first.kind, Kind::Extra);
    assert_eq!((first.r, first.c), (9, 9));
}

/// ⑦ 定向变异：删掉可选键 `source` ⇒ 报 `missing`（纪律 ③ 的可选性）。
#[test]
fn dropping_an_optional_source_is_reported_as_missing() {
    let name = "engine_types";
    let text = fixture_text(name);
    // (1,1) 的 source 整段删掉
    let victim = r#","source":{"src_row":1,"src_col":1,"row_merge":1,"col_merge":1,"extend":16,"sub_cells":1}"#;
    let mutated = text.replacen(victim, "", 1);
    assert_ne!(mutated, text, "变异未生效");
    let r = compare_actual_text(&mutated, name);
    assert!(!r.is_clean());
    let has_source_missing = r.diffs.iter().any(|d| d.dim.as_str() == "source");
    assert!(has_source_missing, "删 source 未被报出：\n{}", r.summary());
}

/// ⑧ 纪律 ③：`calc` 坐标 ≠ `src` 坐标不是错误（`engine_merge` 的 `(2,4)<-src(2,3)`）。
///
/// 自比对必须干净 —— 若对拍器偷偷把「calc 坐标 == src 坐标」当不变量，
/// 这一格会报 mismatch，这里就会红。
#[test]
fn calc_coord_differing_from_src_coord_is_not_an_error() {
    let r = compare_fixture_to_itself("engine_merge");
    assert!(
        r.is_clean(),
        "engine_merge 自比对竟有差异：\n{}",
        r.summary()
    );
    // 确认该夹具确实存在这种格（否则本测试是永真式）
    let g = common::load("engine_merge").grid().unwrap().clone();
    let odd = g
        .cells
        .iter()
        .filter(|c| {
            c.source
                .as_ref()
                .is_some_and(|s| s.src_row != c.r || s.src_col != c.c)
        })
        .count();
    assert!(
        odd > 0,
        "engine_merge 应至少有一格 calc≠src，否则本测试是永真式"
    );
}

/// ⑨ 错误型夹具：形态一致时干净；形态不一致时报出来。
#[test]
fn error_form_compares_class_and_chain_only() {
    let r = compare_fixture_to_itself("engine_cycle");
    assert!(
        r.is_clean(),
        "engine_cycle 自比对竟有差异：\n{}",
        r.summary()
    );
    assert!(r.expected_dims.is_none(), "错误形态没有网格尺寸");
}

/// ⑩ 新增的 head 通道比对必须咬得住：改一个 `left_head.src_row` ⇒ 报 `left_head` 上的 mismatch。
///
/// 挑 `engine_expr` 的 (2,2)：它是**非零** head（`left_head = (2,1)`），
/// 所以这条不是"在全是 (0,0) 的数据上验证 (0,0)"。
#[test]
fn a_changed_head_reference_is_reported_as_mismatch() {
    let name = "engine_expr";
    let text = fixture_text(name);
    let anchor = r#""r":2,"c":2,"value":100,"type":"int","left_head":{"src_row":2,"src_col":1}"#;
    assert!(
        text.contains(anchor),
        "锚点未找到：(2,2) 应带非零 left_head"
    );
    let mutated = text.replacen(
        anchor,
        r#""r":2,"c":2,"value":100,"type":"int","left_head":{"src_row":9,"src_col":1}"#,
        1,
    );
    assert_ne!(mutated, text, "变异未生效");
    let r = compare_actual_text(&mutated, name);
    let dims: Vec<&str> = r.diffs.iter().map(|d| d.dim.as_str()).collect();
    assert!(dims.contains(&"left_head"), "应报 left_head 差异：{dims:?}");
    let hit = r
        .diffs
        .iter()
        .find(|d| d.dim.as_str() == "left_head")
        .unwrap();
    assert_eq!((hit.r, hit.c), (2, 2), "应精确定位到 (2,2)");
    assert_eq!(hit.detail, "src_row", "应指明是 src_row 这一项");
    assert_eq!(hit.expected, "2");
    assert_eq!(hit.actual, "9");
}

/// ⑪ 删掉一个 head 引用 ⇒ 报 `missing`（模型里是 `Option`，但夹具写了的就必须对上）。
#[test]
fn a_deleted_head_reference_is_reported_as_missing() {
    let name = "engine_expr";
    let text = fixture_text(name);
    let victim = r#""left_head":{"src_row":2,"src_col":1},"#;
    assert!(text.contains(victim), "锚点未找到");
    let mutated = text.replacen(victim, "", 1);
    let r = compare_actual_text(&mutated, name);
    let hit = r.diffs.iter().find(|d| d.dim.as_str() == "left_head");
    assert_eq!(
        hit.map(|d| d.kind),
        Some(Kind::Missing),
        "\n{}",
        r.summary()
    );
    assert_eq!(hit.unwrap().detail, "夹具带该 head 引用，引擎未产出");
}
