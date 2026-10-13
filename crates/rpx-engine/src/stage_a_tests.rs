//! 阶段 A 的单测。契约见 `docs/m2-engine-design.md` §3。
//!
//! 断言全部从**产品夹具现推**，不写死数字：模板尺寸取 `expected.json` 的
//! `template_rows`/`template_cols`，合并跨度取 `calculated` 段的 `source`。


use crate::arena::CellId;
use crate::extend::ExtendMode;
use crate::stage_a::{StageA, ROW_TYPE_GROUP_HEADER};

/// 读一个夹具的 `.rpx` 并跑阶段 A。
///
/// 走 `rpx_core::open(File)`（与 `rpx-cli print` 同一条路径），**不能**直接把文件字节
/// 喂给 `ReportDefine::fill` —— 后者只吃 `ReportDefine.serialize()` 那一段 payload，
/// 整个 `.rpx` 文件还带外层容器（会报 "matrix dims exceed payload capacity"）。
fn stage_a_for(rpx: &str) -> StageA {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rpx);
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("打开 {rpx} 失败：{e}"));
    let rd = rpx_core::open(file).unwrap_or_else(|e| panic!("解析 {rpx} 失败：{e}"));
    StageA::build(&rd)
}

/// 从夹具的 `expected.json` 取模板尺寸（0 标度 JSON 文本扫描，避免引入依赖）。
fn template_dims(name: &str) -> (i32, i32) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(format!("{name}.expected.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("读取 {name} 失败：{e}"));
    let rows = int_after(&text, "\"template_rows\":").expect("template_rows");
    let cols = int_after(&text, "\"template_cols\":").expect("template_cols");
    (rows, cols)
}

fn int_after(text: &str, key: &str) -> Option<i32> {
    let at = text.find(key)? + key.len();
    let rest = &text[at..];
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

// ==================== 尺寸口径 ====================

#[test]
fn stage_a_dimensions_are_the_scaffold_inclusive_template_size() {
    // ⚠ **单位陷阱**（本项目已栽过一次）：
    //   Java      irow = rd.getRowCount()+1          （ExtCellSet.java:144-145）
    //   本结构    CalcMatrix::row_size()             = 含 0 号脚手架的矩阵行数
    //   夹具      expected.json 的 template_rows    = **逻辑**行数，**不含**脚手架
    // 所以正确关系是 `row_size() == template_rows + 1`，不是相等。
    // 旁证：`rpx-cli print engine_sum.rpx` → `matrix 7x10 (logical 6x9)`，
    // 而 engine_sum.expected.json 的 template_rows/cols = 6 / 9。
    for name in ["engine_sum", "engine_ext", "engine_merge", "engine_pager"] {
        let (tr, tc) = template_dims(name);
        let s = stage_a_for(&format!("{name}.rpx"));
        assert_eq!(
            (s.calc.row_size(), s.calc.col_size()),
            (tr + 1, tc + 1),
            "{name}: 阶段 A 矩阵尺寸 = 模板逻辑尺寸 + 1（含 0 号脚手架）"
        );
        // 而 row_count() 恰好回到夹具的逻辑口径
        assert_eq!(s.calc.row_count(), tr, "{name}: row_count 应等于 template_rows");
        assert_eq!(s.calc.col_count(), tc, "{name}: col_count 应等于 template_cols");
    }
}

// ==================== cell00 ====================

#[test]
fn cell00_is_the_first_arena_slot_and_sits_at_the_origin() {
    // ExtCellSet.java:148-150：cell00 是 ExtExtendCell，同时进 _$26 与 _$25
    let s = stage_a_for("engine_sum.rpx");
    assert_eq!(s.calc.get(0, 0), CellId::ROOT, "cell00 必须是 ROOT");
    assert!(s.calc.cell(CellId::ROOT).is_some(), "ROOT 必须在 arena 里");
}

// ==================== 行列头 ====================

#[test]
fn row_and_col_headers_occupy_the_zero_axis() {
    // :151-159 行头在 (i,0)；:160-165 列头在 (0,j)
    let s = stage_a_for("engine_sum.rpx");
    let (rows, cols) = template_dims("engine_sum");
    for i in 1..rows {
        assert!(s.calc.get(i, 0).is_some(), "行头 ({i},0) 应存在");
    }
    for j in 1..cols {
        assert!(s.calc.get(0, j).is_some(), "列头 (0,{j}) 应存在");
    }
}

#[test]
fn group_header_flag_latches_only_from_template_minus90_rows() {
    // :151-155 —— 构造期扫模板，有任一 -90 行就置位；与计算无关。
    // engine_pager 的第 2 行被标了 -90（EngineModels.pagerFixture）。
    let with = stage_a_for("engine_pager.rpx");
    assert!(
        with.has_group_header,
        "engine_pager 模板含 -90 分组头行，标志必须为真"
    );
    let without = stage_a_for("engine_sum.rpx");
    assert!(
        !without.has_group_header,
        "engine_sum 模板无 -90 行，标志必须为假"
    );
    assert_eq!(ROW_TYPE_GROUP_HEADER, -90, "常量值对齐 IRowCell.java:21");
}

// ==================== 合并区 ====================

#[test]
fn a_merge_anchor_records_its_full_span() {
    // :177 —— 锚点建格时传入 `endRow-i+1` / `endCol-j+1`，即整个矩形跨度。
    // engine_merge 的 A2 合并 A2:A3 ⇒ 跨度 row_merge = 2。
    // 跨度存在**引擎矩阵**（模板态属性），不是计算态格的值。
    let s = stage_a_for("engine_merge.rpx");
    let anchor = s.engine.get(2, 1).expect("(2,1) 在模板内");
    assert_eq!(anchor.src_row, 2);
    assert_eq!(anchor.src_col, 1);
    assert_eq!(
        anchor.row_merge, 2,
        "A2:A3 的锚点应记录 row_merge=2，实测 {}",
        anchor.row_merge
    );
    assert_eq!(anchor.col_merge, 1, "这是纵向合并，col_merge 应为 1");
}

#[test]
fn engine_merge_non_anchor_slot_keeps_its_own_template_origin() {
    // §3 的实测锚点：engine_merge 的 (3,1) 的 source 是**它自己**（rm=1, ext=16），
    // 不是锚点 (2,1) —— 这正是"后续覆盖"在起作用。
    // 这里断言引擎矩阵给出的模板出处：src_row 应为 3 而非 2。
    let s = stage_a_for("engine_merge.rpx");
    let at_3_1 = s.engine.get(3, 1).expect("(3,1) 应在模板尺寸内");
    let at_2_1 = s.engine.get(2, 1).expect("(2,1) 应在模板尺寸内");
    // (3,1) 若被自己的普通格覆盖，src_row == 3；若沿用锚点，src_row == 2。
    assert_eq!(
        at_3_1.src_row, 3,
        "(3,1) 的模板出处应是自己（浅合并的后续覆盖），实测 got {}",
        at_3_1.src_row
    );
    assert_eq!(at_2_1.src_row, 2, "(2,1) 是合并锚点，出处是自己");
}

#[test]
fn source_is_absent_outside_template_bounds() {
    // 双矩阵契约：source != null <=> 落在模板尺寸内（§0.1）。
    // 阶段 A 还没扩展，所以这里验证"界内一定有、界外一定没有"。
    let s = stage_a_for("engine_ext.rpx");
    let (tr, tc) = template_dims("engine_ext");
    assert!(s.engine.get(tr, tc).is_some(), "右下角 (tr,tc) 应有 source");
    assert!(s.engine.get(tr + 1, tc).is_none(), "越界行必须没有 source");
    assert!(s.engine.get(tr, tc + 1).is_none(), "越界列必须没有 source");
}

#[test]
fn extend_mode_is_carried_over_from_the_template_cell() {
    // 阶段 A 照抄模板声明值；16 -> 17/18/19 的改写发生在阶段 B（§4.3）。
    let s = stage_a_for("engine_ext.rpx");
    let declared = s.engine.get(2, 1).expect("(2,1) 在模板内");
    assert_eq!(
        declared.extend,
        ExtendMode::None17,
        "engine_ext 的 (2,1) 声明 17，阶段 A 应原样保留"
    );
}

// ==================== 未知/空槽 ====================

#[test]
fn blank_template_slots_stay_none() {
    // :169 `cell2 == null` -> continue：模板空槽在计算矩阵里保持 NONE。
    let s = stage_a_for("engine_types.rpx"); // 4x4 全填
    assert!(s.calc.get(1, 1).is_some());
    // 不存在的坐标一律 NONE（越界）
    assert_eq!(s.calc.get(99, 99), CellId::NONE);
    assert_eq!(s.calc.get(-1, 0), CellId::NONE);
}