//! M2-48：**引擎输出 vs 产品夹具**逐格对拍（主验收）。
//!
//! # 这条链路验收什么
//!
//! 对 12 个引擎夹具各跑一遍：「`.rpx` 模板 + `.csv` 数据集 → `rpx_engine::calculate()`
//! → `emit::render_document()` 渲染成 `expected.json` 同 schema 的 JSON」，再交给
//! [`common::compare_actual_text`] 与夹具**逐格**比（值 / 类型 / `disp_value` /
//! `source` 的 6 个子字段 / `sub_values` / `left_head` / `top_head` / 网格尺寸 /
//! **`merged.cells` 的合并跨度**）。
//! 计算态的唯一事实源是产品引擎自己产出的 `tests/fixtures/engine_*.expected.json`
//! （`GenEngineFixtures.java`），禁止手搓期望值。
//!
//! # 全夹具对拍
//!
//! 合并、分页与 M2-22 属性表达式计算均进入同一对拍链路：`merged.cells`、
//! `merged.page_count`、`pagination`（纸张参数及每页字段）全部逐项比较。`engine_rowheight`
//! 的 17 个计算态行高由行属性表达式求出，驱动产品基准中的两页 / 8+9 行切分。
//!
//! 不再设置夹具级豁免：`Dim::Value` / `Dim::Type` / `Dim::Source` 上的 Missing / Extra，
//! 以及任一分页/合并差异都必须使对拍失败。
//!
//! # 为什么本文件必须在实现落地前先写好
//!
//! 团队约定"验收测试先行"：先把判据钉死，实现来对齐它，而不是实现写完后
//! 拿测试去迁就实现。故本文件写于 `calculate()` / `dataset::load_datasets` /
//! `emit::render_calculated` 落地之前 —— **编译不过属预期**，落地后应转绿。

mod common;

use common::{all_names, compare_actual_text, Dim, Kind};
use std::path::PathBuf;

/// 夹具目录口径与 `tests/units_contract.rs:45-47` / `tests/common/loader.rs:32-34` 一致。
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// 读夹具的**原始文本**（用于把夹具当"必定正确"的合成引擎输出做变异，见第 2 组）。
fn fixture_text(name: &str) -> String {
    let path = fixtures_dir().join(format!("{name}.expected.json"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("读 {} 失败：{e}", path.display()))
}

/// 解析 `.rpx`（产品 writer 产出的真实字节），写法对齐 `tests/units_contract.rs:55-59`。
fn open_rpx(name: &str) -> rpx_core::ReportDefine {
    let path = fixtures_dir().join(format!("{name}.rpx"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("读 {} 失败：{e}", path.display()));
    rpx_core::open_bytes(&bytes).unwrap_or_else(|e| panic!("解析 {} 失败：{e}", path.display()))
}

/// 引擎错误 → `(class, chain)`。
///
/// `class` 的取值来自夹具地面真值 `engine_cycle.expected.json` 的
/// `calculated.error.class == "com.raqsoft.commonl.ReportError"`；`chain` 直接取
/// [`rpx_engine::EngineError::CircularRef`] 自带的串（`error.rs:18-19`）。
/// 注意：**这不代表引擎侧已把 class 定死** —— 若日后引擎提供 `class_name()`，
/// 本函数应改为委托它；当前的硬编码会被第 3 组测试与夹具比对咬住（不匹配即红）。
fn error_class_chain(e: &rpx_engine::EngineError) -> (String, Option<String>) {
    use rpx_engine::EngineError as E;
    match e {
        E::CircularRef { chain } => (
            "com.raqsoft.commonl.ReportError".to_string(),
            Some(chain.clone()),
        ),
        // 未建模的错误没有可对上的 fixture class，直接响铃而不是猜。
        other => panic!("未建模的引擎错误，无法映射 class：{other}"),
    }
}

/// 跑一个夹具，产出**引擎侧文本**（`Ok` → 渲染网格；`Err` → 渲染错误段）。
fn engine_text(name: &str) -> String {
    let rd = open_rpx(name);
    let ds = rpx_engine::dataset::load_datasets(&fixtures_dir(), &rd)
        .unwrap_or_else(|e| panic!("{name}: load_datasets 失败：{e}"));
    match rpx_engine::calculate(&rd, &ds) {
        Ok(grid) => rpx_engine::emit::render_document(&grid),
        Err(e) => {
            let (class, chain) = error_class_chain(&e);
            rpx_engine::emit::render_calculated_error(&class, chain.as_deref())
        }
    }
}

// =====================================================================
// 第 1 组：12 夹具逐格对拍（主验收）
// =====================================================================

/// 每个夹具：引擎文本 vs 夹具逐格比，只放行 `engine_rowheight` 的分页两维。
///
/// **为什么不是空跑**：夹具网格非空（下面有断言），且"干净"是在真的比过
/// 值/类型/source/head/尺寸/**合并跨度**/**分页**之后才成立的；
/// 豁免之外任何一条差异都会让本测试红。
#[test]
fn engine_output_matches_every_fixture_cell_by_cell() {
    for name in all_names() {
        let report = compare_actual_text(&engine_text(name), name);

        // ① 网格尺寸必须两侧一致（错误型夹具两侧都 None）。尺寸差异不在豁免之列。
        assert_eq!(
            report.expected_dims,
            report.actual_dims,
            "{name}: 计算态网格尺寸不一致\n{}",
            report.summary()
        );

        // ② 非空跑钉子：网格型夹具必须有格可比，否则"干净"可能是"两边都空"。
        let fx = common::load(name);
        if let Some(g) = fx.grid() {
            assert!(!g.cells.is_empty(), "{name}: 夹具网格为空，'干净'是空跑");
        }

        // 全部差异均为阻断项，包括值、类型、结构、合并与分页。
        for d in &report.diffs {
            let expensive = matches!(d.dim, Dim::Value | Dim::Type | Dim::Source)
                && matches!(d.kind, Kind::Missing | Kind::Extra);
            assert!(!expensive, "{name}: 扩展结构差异：{d}");
        }
        assert!(
            report.diffs.is_empty(),
            "夹具 {name} 必须逐格、逐页零差异\n{}",
            report.summary()
        );
    }
}

// =====================================================================
// 第 2 组：非空跑钉子 —— 证明上面的对拍不是恒真式
// =====================================================================

/// 拿一份"必定正确"的引擎输出改一处 ⇒ 对拍**必须**报出差异并定位到坐标/维度。
///
/// **为什么不是空跑**：先断言未变异的文本干净（正控制），再断言变异后红
/// （负控制）——两者同时成立才说明对拍链路有判别力，而不是"怎么比都通过"。
///
/// 用夹具原文充当合成引擎输出：它与 `expected.json` 同 schema，且正是引擎被要求
/// 产出的那份文本；在 `emit` 尚未落地时这是唯一能构造"已知正确"文本的来源。
#[test]
fn mutation_of_a_known_good_output_is_detected() {
    let name = "engine_types";
    let good = fixture_text(name);
    // 正控制：未变异文本必须干净，否则下面的负控制没有意义。
    assert!(
        compare_actual_text(&good, name).is_clean(),
        "前提不成立：原文本竟不干净"
    );

    // 把 (2,1) 的 value 100 改成 999（该格锚点见 engine_types.expected.json `cells`）。
    let bad = good.replacen(
        r#"{"r":2,"c":1,"value":100,"type":"int""#,
        r#"{"r":2,"c":1,"value":999,"type":"int""#,
        1,
    );
    assert_ne!(bad, good, "变异未生效（锚点未命中）");

    let r = compare_actual_text(&bad, name);
    let hit = r.first_error().expect("改值必须被报出差异");
    assert_eq!(hit.kind, Kind::Mismatch, "\n{}", r.summary());
    assert_eq!(hit.dim, Dim::Value, "\n{}", r.summary());
    assert_eq!(
        (hit.r, hit.c),
        (2, 1),
        "应精确定位到改动的格\n{}",
        r.summary()
    );
}

/// 整表为空的引擎输出 ⇒ 对拍**必须**报出大量 `Missing`（且条数==夹具格数，不是位置数）。
///
/// **为什么不是空跑**：`engine_sum` 计算态是 8×9=72 个位置、却只有 36 个非空格
/// （对拍器纪律 ②：`cells` 稀疏）。空 `cells` 的引擎输出必须产出**恰好 36** 条
/// `Dim::Value` 的 `Missing` —— 既证明"真的整表空"会被抓，又证明对拍器**没有**
/// 退化成按 72 个位置逐点比（否则会是 72）。这两个方向一起才钉住这条纪律。
#[test]
fn a_cells_free_grid_is_reported_as_mass_missing() {
    let g = common::load("engine_sum")
        .grid()
        .expect("engine_sum 是网格形态")
        .clone();
    let positions = (g.rows * g.cols) as usize;
    assert!(
        positions > g.cells.len(),
        "前提：网格是稀疏的（位置数 > 格数）"
    );

    let empty = format!(
        r#"{{"calculated":{{"rows":{},"cols":{},"cells":[]}}}}"#,
        g.rows, g.cols
    );
    let r = compare_actual_text(&empty, "engine_sum");
    let value_missing = r
        .diffs
        .iter()
        .filter(|d| d.kind == Kind::Missing && d.dim == Dim::Value)
        .count();
    assert_eq!(
        value_missing,
        g.cells.len(),
        "空 cells 应对夹具的每个非空格各报一条 Missing（{})，实得 {value_missing}\n{}",
        g.cells.len(),
        r.summary()
    );
    assert!(
        r.counts().0 >= g.cells.len(),
        "总缺失数不应少于夹具格数\n{}",
        r.summary()
    );
}

/// 渲染器输出必须是**合法 JSON**，且对拍入口对同一文本两次结果一致（纯函数性）。
///
/// **为什么不是空跑**：`render_calculated` 若吐出非法 JSON，`compare_actual_text`
/// 内部会 panic（`compare/mod.rs:78-80`）——那条 panic 无法区分"渲染器坏了"与
/// "对拍器坏了"；这里先用公开 parser 显式解析一次，把问题定位到渲染器。
#[test]
fn renderer_emits_parseable_json() {
    let name = "engine_types";
    let rd = open_rpx(name);
    let ds = rpx_engine::dataset::load_datasets(&fixtures_dir(), &rd).expect("load_datasets");
    let grid = rpx_engine::calculate(&rd, &ds).expect("engine_types 应算出网格");
    let text = rpx_engine::emit::render_calculated(&grid);
    common::json::parse(&text)
        .unwrap_or_else(|e| panic!("render_calculated 产出非法 JSON：{e}\n{text}"));

    // 同一文本两次对拍结果一致 ⇒ 证明入口无隐藏状态，不是"第一次比完就变"。
    let a = compare_actual_text(&text, name);
    let b = compare_actual_text(&text, name);
    assert_eq!(a.diffs.len(), b.diffs.len(), "对拍入口应是无状态纯函数");
}

// =====================================================================
// 第 3 组：engine_cycle 的错误形态
// =====================================================================

/// `engine_cycle`：`calculate` 必须返 `Err`，错误对上夹具 `class`/`chain`，
/// 且**没有网格输出**（夹具 `expected_dims` 为 `None` ⇒ 引擎侧也必须 `None`）。
///
/// **为什么不是空跑**：夹具的 `chain` 是实测值 `"B2-C2-B2-C2"`
/// （`engine_cycle.expected.json` 的 `calculated.error.chain`，来源
/// `GenEngineFixtures.java:206-215`）。这里既对夹具值断言、又硬钉这一个常量，
/// 所以"能在错误上挂个随便的串"过不了。
/// `engine_rowheight` 曾是分页豁免；M2-22 落地后必须全段零差异，且不可重新加回白名单。
#[test]
fn rowheight_property_closes_the_last_pagination_diff() {
    let report = compare_actual_text(&engine_text("engine_rowheight"), "engine_rowheight");
    assert!(
        report.diffs.is_empty(),
        "engine_rowheight 不应再有分页豁免或遗留差异\n{}",
        report.summary()
    );
}

// =====================================================================
// 第 4 组：M2-08 合并结果的硬钉
// =====================================================================

/// `engine_samemerge`：列 A 的 `A,A,B,B,C` 必须被 `mergeSame=98` 收成两段 `rm=2`，
/// 且**延续槽也报锚点的跨度**（4 行 = 2 锚点 × 2 槽），末尾独一份的 `C` 不进 `merged`。
///
/// **为什么不是空跑**：这里同时钉住三件事，任一被改坏都会红 ——
/// ① 合并**发生了**（若 `merge_same` 变成空转，本表会空）；
/// ② 锚点是 `(2,1)`/`(4,1)` 而不是 `(2,1)`/`(3,1)`（判据是「同值相邻」不是「相邻就并」）；
/// ③ 延续槽 `(3,1)`/`(5,1)` 报的是**锚点的** `rm=2` —— 这正是 `_$26.set()` 只改槽位映射、
///    不改锚点坐标的直接后果，也是 `merged` 段与 `source` 段分家的那一半。
///
/// 期望值来自 `tests/fixtures/engine_samemerge.expected.json` 的 `merged.cells`
/// （产品产出，非手搓）。
#[test]
fn samemerge_collapses_equal_adjacent_values_into_row_merge_two() {
    let name = "engine_samemerge";
    let rd = open_rpx(name);
    let ds = rpx_engine::dataset::load_datasets(&fixtures_dir(), &rd).expect("load_datasets");
    let grid = rpx_engine::calculate(&rd, &ds).expect("engine_samemerge 应算出网格");

    assert_eq!(grid.page_count, 1, "该夹具是单页（合并夹具的前提）");
    let got: Vec<(i32, i32, i32, i32)> = grid
        .merged
        .iter()
        .map(|m| (m.r, m.c, m.row_merge, m.col_merge))
        .collect();
    assert_eq!(
        got,
        vec![(2, 1, 2, 1), (3, 1, 2, 1), (4, 1, 2, 1), (5, 1, 2, 1)],
        "A,A,B,B 两段各含锚点与延续槽，C 行不合并"
    );
}

#[test]
fn cycle_fixture_is_error_with_expected_class_and_chain() {
    let name = "engine_cycle";
    let rd = open_rpx(name);
    let ds = rpx_engine::dataset::load_datasets(&fixtures_dir(), &rd).expect("load_datasets");
    let err = rpx_engine::calculate(&rd, &ds).expect_err("engine_cycle 必须报错，绝不产出网格");

    let (class, chain) = error_class_chain(&err);
    let expect = common::load(name)
        .error()
        .expect("engine_cycle 是 error 形态")
        .clone();
    assert_eq!(class, expect.class, "错误 class 必须对上夹具");
    assert_eq!(
        chain.as_deref(),
        expect.chain.as_deref(),
        "chain 必须对上夹具"
    );
    // 硬钉：循环引用链逐字必须是 B2-C2-B2-C2。
    assert_eq!(
        chain.as_deref(),
        Some("B2-C2-B2-C2"),
        "循环引用链必须是夹具实测值"
    );

    let text = rpx_engine::emit::render_calculated_error(&class, chain.as_deref());
    let r = compare_actual_text(&text, name);
    assert!(r.expected_dims.is_none(), "夹具是错误形态，没有网格尺寸");
    assert!(r.actual_dims.is_none(), "引擎侧也不得产出网格尺寸");
    assert!(
        r.diffs.is_empty(),
        "engine_cycle 错误段对拍应干净\n{}",
        r.summary()
    );
}
