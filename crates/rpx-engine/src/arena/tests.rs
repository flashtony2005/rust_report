//! `arena` 的单测。算法契约见 `docs/m2-engine-design.md` §0.1/§1 与 `docs/format-contract.md` 附件 I。
//!
//! 引擎矩阵（含"两张矩阵不可合并"的反例）拆到子模块 [`engine_matrix`]，
//! 只为守住「单文件 ≤300 行」的门禁。

mod engine_matrix;

use super::*;
use crate::extend::ExtendMode;
use crate::value::Value;

/// 造一个带出处的格。
fn cell_with_src(row: i32, col: i32, src_row: i32, src_col: i32, ext: ExtendMode) -> CalcCell {
    CalcCell {
        origin: (src_row, src_col),
        row,
        col,
        value: Value::Text(format!("{src_row},{src_col}")),
        disp_value: None,
        src: Some(SourceRef::new(src_row, src_col, ext)),
        sub_values: Vec::new(),
        left_head: None,
        top_head: None,
    }
}

// ==================== CellId ====================

#[test]
fn cell_id_sentinels_are_distinct_and_copyable() {
    assert_ne!(CellId::NONE, CellId::ROOT);
    assert!(!CellId::NONE.is_some());
    assert!(CellId::NONE.is_none());
    assert!(CellId::ROOT.is_some());
    assert!(!CellId::ROOT.is_none());
    let a = CellId(7);
    let b = a; // Copy
    assert_eq!(a, b);
}

#[test]
fn none_has_no_arena_index_but_root_is_index_zero() {
    // 这条就是"leftHead == null 与 leftHead == cell00 不是一回事"的底座：
    // NONE 没有下标，ROOT 指向 arena 第 0 格，二者不可混淆。
    assert_eq!(CellId::NONE.index(), None);
    assert_eq!(CellId::ROOT.index(), Some(0));
    assert_eq!(CellId(9).index(), Some(9));
}

// ==================== SourceRef ====================

#[test]
fn source_ref_defaults_to_unmerged() {
    let s = SourceRef::new(2, 1, ExtendMode::Vertical18);
    assert_eq!((s.row_merge, s.col_merge), (1, 1));
    assert!(!s.is_merged());
}

#[test]
fn merge_flag_follows_either_axis() {
    let mut s = SourceRef::new(2, 1, ExtendMode::Vertical18);
    s.row_merge = 2;
    assert!(s.is_merged());
    let mut t = SourceRef::new(2, 3, ExtendMode::None17);
    t.col_merge = 2;
    assert!(t.is_merged());
}

// ==================== CalcMatrix：尺寸口径 ====================

#[test]
fn calc_matrix_sizes_are_scaffold_inclusive_but_counts_are_not() {
    // rpx_core::ReportDefine.rows 是**含 0 号脚手架**的矩阵行数；
    // 而 expected.json 的 rows 是 getRowCount() = 尺寸 − 1。差 1，别混。
    // 旁证：rpx-cli print tests/fixtures/engine_sum.rpx -> matrix 7x10 (logical 6x9)
    let m = CalcMatrix::new(7, 10);
    assert_eq!((m.row_size(), m.col_size()), (7, 10));
    assert_eq!((m.row_count(), m.col_count()), (6, 9));
}

// ==================== CalcMatrix：槽位 ====================

#[test]
fn calc_matrix_get_set_and_bounds() {
    let mut m = CalcMatrix::new(3, 3);
    assert_eq!(m.get(1, 1), CellId::NONE);
    let id = m.push_cell(CalcCell::bare(1, 1, Value::Int(1)));
    m.set(1, 1, id);
    assert_eq!(m.get(1, 1), id);
    assert_eq!(m.get(0, 0), CellId::NONE); // cell00 槽已存在但尚未填
    m.set(9, 9, id); // 越界忽略
    assert_eq!(m.get(9, 9), CellId::NONE);
    assert!(!m.contains(3, 1));
}

#[test]
fn calc_matrix_cell_lookup_is_none_for_sentinel_and_unknown() {
    let mut m = CalcMatrix::new(2, 2);
    let id = m.push_cell(CalcCell::bare(1, 1, Value::Int(7)));
    assert_eq!(m.cell(id).map(|c| c.value.type_name()), Some("int"));
    assert!(m.cell(CellId::NONE).is_none());
    assert!(m.cell(CellId(99)).is_none());
    assert!(m.cell_mut(CellId::NONE).is_none());
}

// ==================== CalcMatrix：整块铺满 ====================

#[test]
fn fill_rect_puts_one_id_in_every_slot_of_the_half_open_rect() {
    // 对齐 ExtCellSet._$1(startRow,startCol,endRow,endCol,head):706-712
    // —— 半开区间，end 不在内。
    let mut m = CalcMatrix::new(6, 6);
    let id = m.push_cell(cell_with_src(2, 2, 2, 2, ExtendMode::Vertical18));
    m.fill_rect(2, 2, 4, 4, id);
    for (r, c) in [(2, 2), (2, 3), (3, 2), (3, 3)] {
        assert_eq!(m.get(r, c), id, "({r},{c}) 应在矩形内");
    }
    for (r, c) in [(1, 2), (2, 1), (4, 2), (2, 4), (4, 4)] {
        assert_eq!(m.get(r, c), CellId::NONE, "({r},{c}) 不在矩形内");
    }
}

#[test]
fn fill_rect_is_clipped_by_matrix_bounds() {
    let mut m = CalcMatrix::new(3, 3);
    let id = m.push_cell(CalcCell::bare(1, 1, Value::Int(1)));
    m.fill_rect(1, 1, 99, 99, id);
    assert_eq!(m.get(2, 2), id);
    assert_eq!(m.get(0, 0), CellId::NONE);
}

// ==================== CalcMatrix：插行 ====================

#[test]
fn insert_rows_opens_blank_rows_and_shifts_the_tail() {
    // 对齐 Matrix.insertRows；extend_row 用 _$3(endRow, rows*(count-1)) 插在行带之后。
    let mut m = CalcMatrix::new(5, 4); // 0..4 行，0..3 列
    let a = m.push_cell(CalcCell::bare(1, 1, Value::Text("A".into())));
    let b = m.push_cell(CalcCell::bare(3, 1, Value::Text("B".into())));
    m.set(1, 1, a);
    m.set(3, 1, b);

    m.insert_rows(2, 2); // 在 2 前插 2 行 -> 新行占下标 2,3

    assert_eq!(m.row_size(), 7);
    assert_eq!(m.row_count(), 6);
    assert_eq!(m.get(1, 1), a, "在前面的行不动");
    assert_eq!(m.get(5, 1), b, "被推后的行按 +2 平移");
    assert_eq!(m.get(2, 1), CellId::NONE, "新行是空的");
    assert_eq!(m.get(3, 1), CellId::NONE);
}

#[test]
fn insert_rows_ignores_degenerate_arguments() {
    let mut m = CalcMatrix::new(3, 3);
    let id = m.push_cell(CalcCell::bare(1, 1, Value::Int(1)));
    m.set(1, 1, id);
    m.insert_rows(1, 0); // count<=0
    m.insert_rows(1, -1);
    m.insert_rows(-1, 2);
    m.insert_rows(4, 2); // row > rows
    assert_eq!(m.row_size(), 3);
    assert_eq!(m.get(1, 1), id);
}

// ==================== CalcMatrix：插列 ====================

#[test]
fn insert_cols_opens_blank_cols_and_shifts_every_row() {
    let mut m = CalcMatrix::new(4, 4); // 0..3 行，0..3 列
    let a = m.push_cell(CalcCell::bare(1, 1, Value::Text("A".into())));
    let b = m.push_cell(CalcCell::bare(3, 3, Value::Text("B".into())));
    m.set(1, 1, a);
    m.set(3, 3, b);

    m.insert_cols(2, 3); // 在列 2 前插 3 列 -> 新列占下标 2,3,4

    assert_eq!(m.col_size(), 7);
    assert_eq!(m.col_count(), 6);
    assert_eq!(m.get(1, 1), a, "列 1 在插入点之前，不动");
    assert_eq!(m.get(3, 6), b, "列 3 被推后 +3");
    for r in 0..4 {
        for c in 2..=4 {
            assert_eq!(m.get(r, c), CellId::NONE, "新列 ({r},{c}) 应为空");
        }
    }
}

#[test]
fn insert_cols_keeps_the_merge_block_intact_when_inserted_after_it() {
    // 合并块的每个槽都持同一 id；在块**之后**插列不应破坏这一点。
    let mut m = CalcMatrix::new(4, 5);
    let id = m.push_cell(cell_with_src(1, 1, 1, 1, ExtendMode::Horizontal19));
    m.fill_rect(1, 1, 2, 3, id);
    m.insert_cols(3, 2);
    assert_eq!(m.get(1, 1), id);
    assert_eq!(m.get(1, 2), id);
    assert_eq!(m.get(1, 3), CellId::NONE);
}

// ==================== to_grid：稀疏导出 ====================

#[test]
fn a_null_valued_cell_with_a_display_value_is_still_emitted() {
    // ⚠ 判定式是 `value == null && disp_value == null` 才跳过（CalcJson.java:51-53），
    // 即落盘条件是 **||** 不是 **&&**。反例来自第 8 个夹具 engine_disp：
    // (2,3) 的 type="null"、value=null、disp_value="仅显示值" —— 它**在** cells 里。
    let null_cell = CalcCell::bare(2, 3, Value::Null);
    assert!(!null_cell.is_emitted(), "两个都为空才不落盘");

    let disp_only = CalcCell::display_only(2, 3, "仅显示值".into());
    assert!(disp_only.value.is_null());
    assert!(disp_only.is_emitted(), "value 为空但有显示值 ⇒ 必须落盘");

    let normal = CalcCell::bare(2, 1, Value::Text("North".into()));
    assert!(normal.is_emitted());
}

#[test]
fn to_grid_keeps_a_display_only_cell() {
    let mut m = CalcMatrix::new(3, 3); // 数据 2x2
    let id = m.push_cell(CalcCell::display_only(1, 1, "仅显示值".into()));
    m.set(1, 1, id);
    let g = m.to_grid();
    assert_eq!(g.cells.len(), 1);
    let c = g.get(1, 1).expect("有显示值的空格要落盘");
    assert!(c.value.is_null());
    assert_eq!(c.disp_value.as_deref(), Some("仅显示值"));
}

#[test]
fn to_grid_skips_empty_cells_and_reports_calc_coordinates() {
    // cells 是稀疏的：value 与 disp_value 双 null 的格整条不落盘（CalcJson.java:51-53）。
    // 本仓 disp_value 恒为 None，故只按 value 判。
    let mut m = CalcMatrix::new(5, 4); // 数据 4x3
    let a = m.push_cell(CalcCell::bare(1, 1, Value::Int(10)));
    let empty = m.push_cell(CalcCell::bare(2, 1, Value::Null));
    m.set(1, 1, a);
    m.set(2, 1, empty);

    let g = m.to_grid();
    assert_eq!((g.rows, g.cols), (4, 3));
    assert_eq!(g.cells.len(), 1, "空格不落盘");
    assert_eq!(g.get(1, 1).map(|c| c.value.type_name()), Some("int"));
    assert!(g.get(2, 1).is_none(), "空格查不到 —— 是'空'不是'错'");
    assert!(g.get(9, 9).is_none());
}

#[test]
fn to_grid_expands_a_filled_rect_into_one_record_per_slot() {
    // 合并区铺满后，矩形里每个计算态坐标都导出成一条记录 ——
    // 与夹具一致：附件 H.7 说"被合并的格都报整块跨度"。
    // 矩形是**半开**的：fill_rect(2,1,4,3) 覆盖 r∈{2,3} × c∈{1,2}，共 4 槽。
    let mut m = CalcMatrix::new(6, 6);
    let id = m.push_cell(cell_with_src(2, 1, 2, 1, ExtendMode::Vertical18));
    m.cell_mut(id).unwrap().src.as_mut().unwrap().row_merge = 2;
    m.fill_rect(2, 1, 4, 3, id);
    assert_eq!(m.get(3, 2), id, "半开矩形含 (3,2)");
    assert_eq!(m.get(4, 1), CellId::NONE, "半开矩形不含 r=4");

    let g = m.to_grid();
    assert_eq!(g.cells.len(), 4);
    assert_eq!(g.merged_cells().len(), 4, "四个槽都报 row_merge=2");
    for (r, c) in [(2, 1), (2, 2), (3, 1), (3, 2)] {
        let cell = g.get(r, c).expect("槽应有记录");
        assert_eq!((cell.row, cell.col), (r, c), "导出的是计算态坐标");
        assert_eq!(cell.src.unwrap().src_row, 2, "source 仍是模板坐标");
    }
}

// ==================== 两张矩阵不可合并（反例见子模块） ====================
// 该反例需要同时用到 EngineMatrix 与 CalcMatrix，故放在 [`engine_matrix`] 子模块里。
