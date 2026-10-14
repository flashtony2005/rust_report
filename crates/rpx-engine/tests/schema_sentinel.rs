//! **schema 漂移哨兵**：盯住"产品到底写了哪些键"。
//!
//! # 为什么需要它
//!
//! 加载器里的 `expect_keys` 是**白名单**：它只保证"出现的键都在白名单内"，
//! **不保证"该有的键都在"** —— 若某次生成漏写了 `row_cells`（或 `pagination`），
//! 白名单一声不响。更危险的是反向：有人为了"让它过"而**放宽白名单**，
//! 于是产品侧多写/少写键都再也不会被发现。
//!
//! 所以这里**故意不复用** `expect_keys` 的白名单，而是**独立**写一份"产品必须产出"的键清单：
//! * 两者一致时，无事；
//! * 不一致时（无论是夹具漂移还是有人动了白名单），**先在这里红一次**，
//!   逼出一次**显式决策**，而不是顺手改掉。
//!
//! # 断言两层
//!
//! ① **夹具之间彼此一致**：15 个夹具的顶层键集合必须完全相同；
//! ② **且等于本文件写死的清单**：少了键、多了键都红。
//!
//! ① 抓"某个夹具单独漂移"，② 抓"所有夹具一起少某个键"与"白名单被放宽"。

mod common;

use common::json::{Json, Kind};
use common::{all_names, load_raw};

/// 产品**必须**写出的顶层键（12 个）。
///
/// ⚠ `engine_param` 另有一个**条件键** `params`（M3-01 参数注入面，只该出现在
/// 那一个夹具上）—— 不在这份清单里，由 `params_key_is_unique_to_engine_param`
/// 单独断言，见该测试文档里的取舍说明。
///
/// ⛔ 这份清单与 `common/loader.rs` 里的 `expect_keys` 白名单**故意保持两份**：
/// 若共用一份，加键时改一处就全绿，加键就退化成"顺手"动作。分成两份才能红。
const TOP_LEVEL_KEYS: [&str; 12] = [
    "calculated",
    "dataset",
    "dataset_file",
    "formulas",
    "merged",
    "name",
    "pagination",
    "row_cells",
    "template_cols",
    "template_rows",
    "unit",
    "unit_note",
];

/// 网格形态 `calculated` 段的键。
const CALC_GRID_KEYS: [&str; 3] = ["cells", "cols", "rows"];
/// 错误形态 `calculated` 段的键。
const CALC_ERROR_KEYS: [&str; 1] = ["error"];
/// `pagination` 段的键（非错误形态）。
const PAGINATION_KEYS: [&str; 8] = [
    "columns",
    "page_count",
    "pages",
    "paper_h",
    "paper_w",
    "row_num_per_page",
    "x_pages",
    "y_pages",
];
/// `merged` 段的键（非错误形态）。
const MERGED_KEYS: [&str; 2] = ["cells", "page_count"];
/// `calculated.error` 的键。
const ERROR_KEYS: [&str; 5] = ["cause_class", "cause_message", "chain", "class", "message"];

/// 取对象的键并排序；非对象则带上下文 panic。
fn keys_of(j: &Json, ctx: &str) -> Vec<String> {
    assert!(
        matches!(j.kind, Kind::Obj(_)),
        "{ctx} 应为 JSON 对象，实际是 {}",
        j.kind_name()
    );
    let mut ks: Vec<String> = j.obj().iter().map(|(k, _)| k.clone()).collect();
    ks.sort();
    ks
}

/// 取可空段的键集：`null` / 缺键 ⇒ 空向量。
fn keys_of_opt(root: &Json, key: &str, ctx: &str) -> Vec<String> {
    match root.get(key) {
        None => Vec::new(),
        Some(v) if v.is_null() => Vec::new(),
        Some(v) => keys_of(v, ctx),
    }
}

/// 该夹具是否为错误形态（据**原始** JSON 判断，不依赖类型化加载器 ——
/// 哨兵必须独立于被它监视的那层）。
fn is_error_form(root: &Json) -> bool {
    root.get("calculated")
        .is_some_and(|c| c.get("error").is_some())
}

/// ① + ②：顶层键集合 —— **无参夹具**之间一致，且等于清单。
///
/// ⚠ M3-01 起 `engine_param` **不在本断言内**：它是唯一带 `params` 键的夹具
/// （参数注入面，`GenEngineFixtures.java` 按夹具名条件写入），由
/// [`params_key_is_unique_to_engine_param`] 单独钉。把它塞进"完全一致"的集合
/// 只有两条路：逼所有夹具都写 `params: []`（15 个 expected.json 全部变动），
/// 或把断言放宽成子集（漂移哨兵的意义就没了）—— 单列是唯一既保不变式又不制造
/// 噪音的选择。
#[test]
fn every_fixture_declares_exactly_the_same_top_level_keys() {
    let expect: Vec<String> = TOP_LEVEL_KEYS.iter().map(|s| s.to_string()).collect();
    let first = all_names()[0];
    let reference = keys_of(&load_raw(first), first);
    assert_eq!(reference, expect, "{first}: 顶层键清单与本文件不符");

    for name in all_names().iter().skip(1) {
        if *name == "engine_param" {
            continue; // 见本测试文档：它多一个 params 键，单独断言
        }
        let got = keys_of(&load_raw(name), name);
        // ① 夹具之间彼此一致
        assert_eq!(
            got, reference,
            "{name}: 顶层键集合与 {first} 不一致 —— 产品侧键集合发生漂移"
        );
        // ② 且等于写死的清单（缺键 / 多键都红）
        assert_eq!(got, expect, "{name}: 顶层键集合与本文件清单不符");
    }
}

/// `params` 键是 `engine_param` **独有**的（M3-01）。
///
/// 钉两条不变式：① 15 个无参夹具的 JSON 里**没有** `params` 键（产品侧是条件
/// 写入，一旦写成无条件，它们的 expected.json 会全部变动 —— 那正是"顺手改一处
/// 就全绿"要防的退化）；② 有参夹具的键集合 = 12 个公共键 + `params`，段内每条
/// 都是 `{name, value}`，且至少一条参数。
#[test]
fn params_key_is_unique_to_engine_param() {
    let mut with_params = 0usize;
    for name in all_names() {
        let root = load_raw(name);
        let Some(v) = root.get("params") else {
            continue;
        };
        assert_eq!(
            name, "engine_param",
            "{name}: 只有 engine_param 该有 params 键"
        );
        assert!(!v.is_null(), "engine_param 的 params 不能是 null");
        let items = v.arr();
        assert!(!items.is_empty(), "engine_param 的 params 不能是空数组");
        for item in items {
            let mut got = keys_of(item, name);
            got.sort();
            assert_eq!(
                got,
                vec!["name".to_string(), "value".to_string()],
                "{name}: params 条目必须是 {{name, value}}"
            );
        }
        with_params += 1;
    }
    assert_eq!(with_params, 1, "应当恰有一个夹具带 params 键");
}

/// `calculated` 段的键：网格形态与错误形态各自固定，且两种形态不混用。
#[test]
fn calculated_section_keys_are_uniform_per_form() {
    let grid: Vec<String> = CALC_GRID_KEYS.iter().map(|s| s.to_string()).collect();
    let err: Vec<String> = CALC_ERROR_KEYS.iter().map(|s| s.to_string()).collect();
    let (mut n_grid, mut n_err) = (0usize, 0usize);
    for name in all_names() {
        let root = load_raw(name);
        let calc = root.get("calculated").expect("顶层必有 calculated");
        let got = keys_of(calc, name);
        if is_error_form(&root) {
            assert_eq!(got, err, "{name}: 错误形态的 calculated 键不符");
            n_err += 1;
        } else {
            assert_eq!(got, grid, "{name}: 网格形态的 calculated 键不符");
            n_grid += 1;
        }
    }
    // 非永真式守卫：两种形态都必须至少各有一个样本，否则"一致"是空的
    assert!(
        n_grid > 0 && n_err > 0,
        "网格形态 {n_grid} 个 / 错误形态 {n_err} 个，两种都要有样本"
    );
}

/// `pagination` / `merged` 段的键：非错误形态固定；错误形态必须**不出现**。
#[test]
fn pagination_and_merged_keys_are_uniform_and_absent_on_error_form() {
    let pag: Vec<String> = PAGINATION_KEYS.iter().map(|s| s.to_string()).collect();
    let mrg: Vec<String> = MERGED_KEYS.iter().map(|s| s.to_string()).collect();
    for name in all_names() {
        let root = load_raw(name);
        let got_pag = keys_of_opt(&root, "pagination", name);
        let got_mrg = keys_of_opt(&root, "merged", name);
        if is_error_form(&root) {
            assert!(got_pag.is_empty(), "{name}: 错误形态不该有 pagination 段");
            assert!(got_mrg.is_empty(), "{name}: 错误形态不该有 merged 段");
        } else {
            assert_eq!(got_pag, pag, "{name}: pagination 键集合不符");
            assert_eq!(got_mrg, mrg, "{name}: merged 键集合不符");
        }
    }
}

/// `calculated.error` 的键：只在错误形态出现，且集合固定。
#[test]
fn error_object_keys_are_uniform_and_only_on_error_form() {
    let expect: Vec<String> = ERROR_KEYS.iter().map(|s| s.to_string()).collect();
    let mut seen = 0usize;
    for name in all_names() {
        let root = load_raw(name);
        let calc = root.get("calculated").expect("顶层必有 calculated");
        match calc.get("error") {
            None => {
                assert!(!is_error_form(&root), "{name}: 形态判定与 error 键不一致");
            }
            Some(e) => {
                assert_eq!(keys_of(e, name), expect, "{name}: error 对象键集合不符");
                seen += 1;
            }
        }
    }
    assert!(seen > 0, "没有错误型夹具 ⇒ 本测试是永真式");
}

/// `cells[]` 每个格的**核心键**必须齐（`source` / `disp_value` 之外的全都必须有）。
///
/// 头图切片给 `cells[]` 加了 `left_head` / `top_head`（每格都有），
/// 于是"格"有了一个**稳定的核心键集** = 下列 6 个；只有 `source` / `disp_value`
/// 是按条件才写的可选键（`CalcJson.java:71-73`、`:74-98`）。
///
/// 这条是"加键要显式"的延续：产品若哪天把 `left_head` 挪成可选（或改名），
/// 这里会先红一次，而不是让下游对拍器静默按缺省处理。
#[test]
fn every_cell_carries_the_core_keys() {
    const CORE: [&str; 6] = ["r", "c", "value", "type", "left_head", "top_head"];
    let mut cells_seen = 0usize;
    for name in all_names() {
        let root = load_raw(name);
        let calc = root.get("calculated").expect("顶层必有 calculated");
        if calc.get("error").is_some() {
            continue; // 错误形态没有格
        }
        for cell in calc.get("cells").expect("网格形态必有 cells").arr() {
            let ks = keys_of(cell, name);
            for key in CORE {
                assert!(
                    ks.iter().any(|k| k == key),
                    "{name}: 某个格缺少核心键 `{key}`（该格键集={ks:?}）"
                );
            }
            // 可选键只允许这两个 —— 出现别的键同样是漂移
            for k in &ks {
                assert!(
                    CORE.contains(&k.as_str()) || k == "source" || k == "disp_value",
                    "{name}: 格上出现了既非核心键、也非已知可选键的 `{k}`"
                );
            }
            cells_seen += 1;
        }
    }
    assert!(cells_seen > 0, "没有遍历到任何格 ⇒ 本测试是永真式");
}

// =====================================================================
// ⛔ 以下按设计**故意不做**键集合一致性检查
// =====================================================================
//
// * **`row_cells[]` 内部**：设计上是**开放**的 —— 产品按需附加键
//   （`engine_rowheight` 就有 `height_exp` / `exp_channel` / `computed_anchor_row` /
//   `computed_row_heights` / `not_covered`）。做一致性断言会把"扩展点"变成"漂移"。
//   未知键的**保存**由 `loader_contract.rs::unknown_row_cell_keys_are_kept_in_extra_not_dropped`
//   单独钉住。
// * **`calculated.cells[]` 的"整体键集"**：键集**故意不齐** —— `disp_value` 与
//   `source` 都是可选键（`CalcJson.java:71-73` 与 `:74-98` 按条件才写）。
//   做"全格键集相等"等于强迫所有格都带上这两个键，把稀疏网格变成稠密网格。
//   ⇒ 但**核心键**（`r`/`c`/`value`/`type`/`left_head`/`top_head`）必须齐，
//   这一半由 `every_cell_carries_the_core_keys` 单独钉住。
