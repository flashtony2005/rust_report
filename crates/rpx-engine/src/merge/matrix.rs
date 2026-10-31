//! `merge_same` / `merge_null` 四趟的**合成矩阵**。
//!
//! 12 个产品夹具覆盖 `mergeSame=98` 的部分形态，完全没有 `mergeNull`、`mergeSame=99/100`
//! 或 `mergeSameMode=120`。本模块直接构造 `HeadState` 并调用两个公开趟函数，避免把
//! 尚未覆盖的 P2/P3/P4 与 `mergeNull` 四趟留给源码阅读。
//!
//! 每条都断言**跨度 + 槽位别名 + 锚点坐标/身份**中与该趟相关的结果；只断「函数不 panic」
//! 不算覆盖。扩展/合并的产品侧夹具仍是端到端对拍，本矩阵只补缺失的方向分支。

use super::*;

/// 造含 0 号脚手架的 `rows × cols` 数据区。第 0 行/列分别是列头/行头。
fn grid(rows: i32, cols: i32, row_types: &[i8]) -> HeadState {
    assert_eq!(row_types.len(), rows as usize);
    let mut g = HeadState::new();
    g.push(HeadCell::new(0, 0, 16, -91));
    for r in 1..=rows {
        g.push(HeadCell::new(r, 0, 16, row_types[(r - 1) as usize]));
    }
    for c in 1..=cols {
        g.push(HeadCell::new(0, c, 16, -79));
    }
    g
}

fn cell(
    g: &mut HeadState,
    row: i32,
    col: i32,
    value: Value,
    merge_same: i8,
    merge_same_mode: i8,
    merge_null: i8,
) -> CellId {
    let mut c = HeadCell::new(row, col, 16, -91);
    c.value = value;
    c.merge_same = merge_same;
    c.merge_same_mode = merge_same_mode;
    c.merge_null = merge_null;
    g.push(c)
}

fn slot(g: &HeadState, row: i32, col: i32) -> CellId {
    super::get(g, row, col).unwrap_or_else(|| panic!("缺少槽 ({row},{col})"))
}

fn spans(g: &HeadState, id: CellId) -> (i32, i32) {
    let c = g.get(id).expect("格 id 有效");
    (c.row_merge, c.col_merge)
}

// =====================================================================
// mergeSame: P1–P4
// =====================================================================

#[test]
fn same_p1_merges_equal_values_downward() {
    let mut g = grid(2, 1, &[-91, -91]);
    let anchor = cell(
        &mut g,
        1,
        1,
        Value::Text("A".into()),
        SAME_V,
        MODE_SOURCE,
        113,
    );
    let _next = cell(
        &mut g,
        2,
        1,
        Value::Text("A".into()),
        SAME_V,
        MODE_SOURCE,
        113,
    );

    merge_same(&mut g);

    assert_eq!(spans(&g, anchor), (2, 1));
    assert_eq!(slot(&g, 1, 1), anchor);
    assert_eq!(slot(&g, 2, 1), anchor);
}

#[test]
fn same_p2_reverse_mode_merges_upward_from_the_bottom_anchor() {
    let mut g = grid(2, 1, &[-91, -91]);
    let top = cell(
        &mut g,
        1,
        1,
        Value::Text("A".into()),
        SAME_V,
        MODE_REVERSE,
        113,
    );
    let _bottom = cell(
        &mut g,
        2,
        1,
        Value::Text("A".into()),
        SAME_V,
        MODE_REVERSE,
        113,
    );

    merge_same(&mut g);

    assert_eq!(spans(&g, top), (2, 1));
    assert_eq!(slot(&g, 1, 1), top);
    assert_eq!(slot(&g, 2, 1), top);
    assert_eq!(g.get(top).unwrap().row, 1, "反向纵合并的最终锚点在首行");
}

#[test]
fn same_p3_merges_equal_values_to_the_right_when_p1_left_max2_below_boundary() {
    // 一行数据使 P1 不写方法级 max2；P3 应从初值 0 进入，而不是误跳过。
    let mut g = grid(1, 2, &[-91]);
    let anchor = cell(
        &mut g,
        1,
        1,
        Value::Text("A".into()),
        SAME_H,
        MODE_SOURCE,
        113,
    );
    let _next = cell(
        &mut g,
        1,
        2,
        Value::Text("A".into()),
        SAME_H,
        MODE_SOURCE,
        113,
    );

    merge_same(&mut g);

    assert_eq!(spans(&g, anchor), (1, 2));
    assert_eq!(slot(&g, 1, 1), anchor);
    assert_eq!(slot(&g, 1, 2), anchor);
}

#[test]
fn same_p4_reverse_mode_merges_leftward_and_moves_the_anchor() {
    let mut g = grid(1, 2, &[-91]);
    let left = cell(
        &mut g,
        1,
        1,
        Value::Text("A".into()),
        SAME_H,
        MODE_REVERSE,
        113,
    );
    let _right = cell(
        &mut g,
        1,
        2,
        Value::Text("A".into()),
        SAME_H,
        MODE_REVERSE,
        113,
    );

    merge_same(&mut g);

    assert_eq!(spans(&g, left), (1, 2));
    assert_eq!(slot(&g, 1, 1), left);
    assert_eq!(slot(&g, 1, 2), left);
    assert_eq!(g.get(left).unwrap().col, 1, "P4 把锚点归到最左格");
}

// =====================================================================
// mergeNull: P1–P4
// =====================================================================

#[test]
fn null_p1_absorbs_an_up_marked_blank_downward() {
    let mut g = grid(2, 1, &[-91, -91]);
    let anchor = cell(&mut g, 1, 1, Value::Text("A".into()), 97, 118, 113);
    let _blank = cell(&mut g, 2, 1, Value::Null, 97, 118, NULL_UP);

    merge_null(&mut g);

    assert_eq!(spans(&g, anchor), (2, 1));
    assert_eq!(slot(&g, 2, 1), anchor);
}

#[test]
fn null_p2_absorbs_a_down_marked_blank_upward_and_reanchors() {
    let mut g = grid(2, 1, &[-91, -91]);
    let _blank = cell(&mut g, 1, 1, Value::Null, 97, 118, NULL_DOWN);
    let anchor = cell(&mut g, 2, 1, Value::Text("A".into()), 97, 118, 113);

    merge_null(&mut g);

    assert_eq!(spans(&g, anchor), (2, 1));
    assert_eq!(slot(&g, 1, 1), anchor);
    assert_eq!(slot(&g, 2, 1), anchor);
    assert_eq!(g.get(anchor).unwrap().row, 1, "P2 把锚点上移到被吸收块");
}

#[test]
fn null_p3_absorbs_a_left_marked_blank_to_the_right() {
    let mut g = grid(1, 2, &[-91]);
    let anchor = cell(&mut g, 1, 1, Value::Text("A".into()), 97, 118, 113);
    let _blank = cell(&mut g, 1, 2, Value::Null, 97, 118, NULL_LEFT);

    merge_null(&mut g);

    assert_eq!(spans(&g, anchor), (1, 2));
    assert_eq!(slot(&g, 1, 2), anchor);
}

#[test]
fn null_p4_absorbs_a_right_marked_blank_to_the_left_and_reanchors() {
    let mut g = grid(1, 2, &[-91]);
    let _blank = cell(&mut g, 1, 1, Value::Null, 97, 118, NULL_RIGHT);
    let anchor = cell(&mut g, 1, 2, Value::Text("A".into()), 97, 118, 113);

    merge_null(&mut g);

    assert_eq!(spans(&g, anchor), (1, 2));
    assert_eq!(slot(&g, 1, 1), anchor);
    assert_eq!(slot(&g, 1, 2), anchor);
    assert_eq!(g.get(anchor).unwrap().col, 1, "P4 把锚点左移到被吸收块");
}

// =====================================================================
// 守卫 / 判据对照组：保证矩阵不是只测「总能合并」
// =====================================================================

#[test]
fn null_passes_do_not_absorb_a_non_null_neighbor_even_with_a_direction_code() {
    let mut g = grid(2, 1, &[-91, -91]);
    let anchor = cell(&mut g, 1, 1, Value::Text("A".into()), 97, 118, 113);
    let neighbor = cell(&mut g, 2, 1, Value::Text("B".into()), 97, 118, NULL_UP);

    merge_null(&mut g);

    assert_eq!(spans(&g, anchor), (1, 1));
    assert_eq!(slot(&g, 2, 1), neighbor);
}

#[test]
fn null_p1_does_not_cross_a_row_type_boundary() {
    let mut g = grid(2, 1, &[-91, -95]);
    let anchor = cell(&mut g, 1, 1, Value::Text("A".into()), 97, 118, 113);
    let blank = cell(&mut g, 2, 1, Value::Null, 97, 118, NULL_UP);

    merge_null(&mut g);

    assert_eq!(spans(&g, anchor), (1, 1));
    assert_eq!(slot(&g, 2, 1), blank);
}

#[test]
fn same_p1_does_not_merge_unequal_values() {
    let mut g = grid(2, 1, &[-91, -91]);
    let anchor = cell(
        &mut g,
        1,
        1,
        Value::Text("A".into()),
        SAME_V,
        MODE_SOURCE,
        113,
    );
    let next = cell(
        &mut g,
        2,
        1,
        Value::Text("B".into()),
        SAME_V,
        MODE_SOURCE,
        113,
    );

    merge_same(&mut g);

    assert_eq!(spans(&g, anchor), (1, 1));
    assert_eq!(slot(&g, 2, 1), next);
}
