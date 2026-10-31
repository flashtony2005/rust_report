//! 引擎矩阵（Java `ExtCellSet._$25`）的单测，含「两张矩阵不可合并」的反例。
//!
//! 契约正文：`docs/m2-engine-design.md` §0.1。

use super::cell_with_src;
use crate::arena::{CalcCell, CalcMatrix, EngineMatrix, SourceRef};
use crate::extend::ExtendMode;
use crate::value::Value;

#[test]
fn engine_matrix_is_empty_until_stage_b_fills_it() {
    let m = EngineMatrix::new(4, 3);
    assert_eq!((m.rows(), m.cols()), (4, 3));
    assert_eq!(m.filled_count(), 0);
    assert_eq!(m.get(1, 1), None);
}

#[test]
fn engine_matrix_has_source_exactly_within_template_bounds() {
    // ⛔ 这是 `source != null ⟺ 1<=r<=template_rows && 1<=c<=template_cols` 的判定式
    // （ExtCellSet.java:623-629；六个网格夹具 fn=fp=0）。
    let mut m = EngineMatrix::new(4, 3);
    for r in 1..=4 {
        for c in 1..=3 {
            m.set(r, c, SourceRef::new(r, c, ExtendMode::Default16));
        }
    }
    assert_eq!(m.filled_count(), 12);
    for r in 1..=4 {
        for c in 1..=3 {
            assert!(m.get(r, c).is_some(), "模板内 ({r},{c}) 必须有 source");
            assert!(m.is_within_template(r, c));
        }
    }
    // 越出模板尺寸 —— engine_ext 的 (1,4)/(1,5)/r>=5 全属这一类
    for (r, c) in [
        (1, 4),
        (1, 5),
        (4, 4),
        (5, 1),
        (8, 7),
        (0, 0),
        (0, 1),
        (1, 0),
    ] {
        assert!(m.get(r, c).is_none(), "模板外 ({r},{c}) 必须没有 source");
        assert!(!m.is_within_template(r, c));
    }
}

#[test]
fn engine_matrix_ignores_out_of_range_writes() {
    let mut m = EngineMatrix::new(2, 2);
    m.set(3, 1, SourceRef::new(3, 1, ExtendMode::Default16));
    m.set(1, 5, SourceRef::new(1, 5, ExtendMode::Default16));
    assert_eq!(m.filled_count(), 0);
}

#[test]
fn engine_matrix_slot_can_be_within_bounds_yet_unfilled() {
    // getSource 还要求 _$25 那个槽非 null（ExtCellSet.java:625 的 `eng != null`），
    // 所以"界内"只是必要条件；未填的槽仍然没有 source。
    let mut m = EngineMatrix::new(3, 3);
    m.set(2, 2, SourceRef::new(2, 2, ExtendMode::None17));
    assert!(m.is_within_template(1, 1));
    assert_eq!(m.get(1, 1), None);
}

#[test]
fn source_must_be_read_from_the_engine_matrix_not_the_calc_matrix() {
    // 反例构造：计算矩阵里 (3,1) 是扩展出来的格（模板只 2 行），
    // 若拿计算矩阵回推出处，就会给它编一个不存在的模板 (3,1)。
    // 这正是"两张矩阵不能合并成一张"的最小证据。
    let mut engine = EngineMatrix::new(2, 3);
    for r in 1..=2 {
        engine.set(r, 1, SourceRef::new(r, 1, ExtendMode::Vertical18));
    }
    let mut calc = CalcMatrix::new(5, 4); // 数据 4x3 —— 比模板高
    let id = calc.push_cell(CalcCell::bare(3, 1, Value::Text("克隆".into())));
    calc.set(3, 1, id);

    assert!(calc.cell(calc.get(3, 1)).is_some(), "计算矩阵里有这一格");
    assert_eq!(
        engine.get(3, 1),
        None,
        "但引擎矩阵里没有模板 (3,1) ⇒ 没有 source"
    );
}

#[test]
fn engine_matrix_keeps_merge_span_of_the_template_cell() {
    // engine_merge：模板 A2 合并 A2:A3 且 extend=18 ⇒ 计算态锚点 source 报 row_merge=2。
    // 跨度是**模板态**的属性，存在引擎矩阵里。
    let mut m = EngineMatrix::new(4, 4);
    let mut src = SourceRef::new(2, 1, ExtendMode::Vertical18);
    src.row_merge = 2;
    m.set(2, 1, src);
    let got = m.get(2, 1).expect("模板内应有 source");
    assert_eq!((got.src_row, got.src_col), (2, 1));
    assert_eq!(got.row_merge, 2);
    assert!(got.is_merged());
    assert!(!cell_with_src(1, 1, 1, 1, ExtendMode::Default16).is_merged());
}
