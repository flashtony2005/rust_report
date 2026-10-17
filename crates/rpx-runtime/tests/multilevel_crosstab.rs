//! 教程 dcjcbb93：两层行头 × 两层列头，及交换两轴后的对称性。
use rpx_engine::{arena::CalcGrid, expr::ds::DataSet, value::Value};
use serde_json::json;

// 手工预期：年份→月份为行，地区→城市为列。
// 同名城市属于不同地区；月份跨年重复；组记录不连续且存在重复事实。
const EXPECTED: [[i32; 5]; 5] = [
    [15, 0, 20, 0, 0],
    [0, 30, 0, 40, 0],
    [50, 0, 0, 70, 0],
    [0, 0, 60, 0, 0],
    [0, 0, 0, 0, 80],
];

fn calculate(transpose: bool, zero: bool) -> CalcGrid {
    let (row_outer, row_inner, col_outer, col_inner) = if transpose {
        ("region", "city", "year", "month")
    } else {
        ("year", "month", "region", "city")
    };
    let doc = json!({
        "version": 1, "grid": {"rows": 3, "cols": 3},
        "cells": [
            {"row": 1, "col": 3, "expr": format!("facts.group({col_outer})"), "extend_mode": "horizontal"},
            {"row": 2, "col": 3, "expr": format!("facts.group({col_inner})"), "extend_mode": "horizontal", "top_head": "C1"},
            {"row": 3, "col": 1, "expr": format!("facts.group({row_outer})"), "extend_mode": "vertical"},
            {"row": 3, "col": 2, "expr": format!("facts.group({row_inner})"), "extend_mode": "vertical", "left_head": "A3"},
            {"row": 3, "col": 3, "expr": if zero {"ifn(facts.sum(amount),0)"} else {"facts.sum(amount)"}, "extend_mode": "none", "left_head": "B3", "top_head": "C2"}
        ]
    });
    let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    let rd = rpx_core::open_bytes(&rpx_core::to_bytes(&rd).unwrap()).unwrap();
    let rows = [
        (2024, 1, "East", "Shared", 10),
        (2024, 1, "West", "Shared", 20),
        (2024, 2, "East", "E2", 30),
        (2024, 2, "West", "W2", 40),
        (2025, 1, "East", "Shared", 50),
        (2025, 2, "West", "Shared", 60),
        (2025, 1, "West", "W2", 70),
        (2024, 1, "East", "Shared", 5),
        (2025, 3, "West", "W3", 80),
    ]
    .map(|(y, m, r, c, a)| {
        vec![
            Value::Int(y),
            Value::Int(m),
            Value::Text(r.into()),
            Value::Text(c.into()),
            Value::Int(a),
        ]
    })
    .to_vec();
    let ds = DataSet::new(
        "facts",
        &["year", "month", "region", "city", "amount"],
        rows,
    );
    rpx_engine::calculate(&rd, &[ds]).unwrap()
}

fn check(g: &CalcGrid, transpose: bool, zero: bool) {
    assert_eq!((g.rows, g.cols), (7, 7));
    let cells = g
        .cells
        .iter()
        .filter(|c| c.origin == (3, 3) && c.is_emitted())
        .collect::<Vec<_>>();
    assert_eq!(
        cells.len(),
        if zero { 25 } else { 8 },
        "空交叉默认不输出值；模板显式 ifn 后输出 0"
    );
    for index in 0..25 {
        let (r, c) = (index / 5, index % 5);
        let expected = if transpose {
            EXPECTED[c][r]
        } else {
            EXPECTED[r][c]
        };
        let value = g
            .get(r as i32 + 3, c as i32 + 3)
            .map(|cell| cell.value.clone())
            .unwrap_or(Value::Null);
        let expected = if expected == 0 && !zero {
            Value::Null
        } else {
            Value::Int(expected)
        };
        assert_eq!(value, expected, "结果格 ({r},{c})");
    }
}

#[test]
fn two_levels_on_both_axes_intersect_all_four_groups() {
    for zero in [false, true] {
        check(&calculate(false, zero), false, zero);
    }
}

#[test]
fn swapping_axes_transposes_the_calculated_matrix() {
    for zero in [false, true] {
        check(&calculate(true, zero), true, zero);
    }
}
