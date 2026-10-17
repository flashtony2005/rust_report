#![cfg(feature = "legacy-rpx")]
//! 教程原式从 JSON 装载到扩展网格的回归；期望值手工计算。
use rpx_engine::{arena::CalcGrid, value::Value};
use serde_json::json;

fn calculate(expressions: &[&str]) -> CalcGrid {
    let cells: Vec<_> = expressions
        .iter()
        .enumerate()
        .map(|(i, expr)| json!({"row":1,"col":i+1,"expr":expr}))
        .collect();
    let rd = rpx_runtime::reportdef::build(
        &json!({
            "grid":{"rows":1,"cols":expressions.len()},"cells":cells
        })
        .to_string(),
    )
    .unwrap();
    rpx_engine::calculate(&rd, &[]).unwrap()
}

fn column(g: &CalcGrid, col: i32) -> Vec<Value> {
    (1..=g.rows)
        .map(|r| g.get(r, col).map_or(Value::Null, |c| c.value.clone()))
        .collect()
}

#[test]
fn rank_count_and_absolute_coordinate_run_after_real_extension() {
    let g = calculate(&[
        "to(1,4)",
        "count(A1[`0]{A1>$A1})+1",
        "A1[1]-A1",
        "&A1",
        "seq(A1)",
    ]);
    assert_eq!(
        column(&g, 2),
        vec![Value::Int(4), Value::Int(3), Value::Int(2), Value::Int(1)]
    );
    assert_eq!(
        column(&g, 3),
        vec![
            Value::Int(0),
            Value::Int(-1),
            Value::Int(-2),
            Value::Int(-3)
        ]
    );
    assert_eq!(column(&g, 4), column(&g, 1));
    assert_eq!(column(&g, 5), column(&g, 1));
}

#[test]
fn expression_aggregate_and_lazy_condition_use_the_cellset_context() {
    let g = calculate(&[
        "to(1,4)",
        "A1*10",
        "sum(A1*B1,A1[`0]{})",
        "if(A1==1,0,B1/B1[-1])",
    ]);
    assert_eq!(column(&g, 3), vec![Value::Int(300); 4]);
    assert_eq!(
        column(&g, 4),
        vec![
            Value::Int(0),
            Value::Double(2.0),
            Value::Double(1.5),
            Value::Double(4.0 / 3.0)
        ]
    );
}

#[test]
fn padding_range_and_number_conversion_have_defined_boundaries() {
    let g = calculate(&["to(1,20-17%20)", "int('42.9')+&A1"]);
    assert_eq!(g.rows, 3);
    assert_eq!(
        column(&g, 2),
        vec![Value::Int(43), Value::Int(44), Value::Int(45)]
    );
    let g = calculate(&["count(to(1,0))"]);
    assert_eq!(column(&g, 1), vec![Value::Int(0)]);
}

#[test]
fn cumulative_column_starts_with_null_previous_row() {
    let g = calculate(&["to(1,4)", "A1+B1[-1]"]);
    assert_eq!(
        column(&g, 2),
        vec![Value::Int(1), Value::Int(3), Value::Int(6), Value::Int(10)]
    );
}

#[test]
fn group_sequence_restarts_while_source_sequence_remains_continuous() {
    let rd = rpx_runtime::reportdef::build(
        &json!({
            "grid":{"rows":1,"cols":4},"cells":[
                {"row":1,"col":1,"expr":"ds1.group(team)"},
                {"row":1,"col":2,"expr":"ds1.select(amount)"},
                {"row":1,"col":3,"expr":"&B1"},
                {"row":1,"col":4,"expr":"seq(B1)"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let ds = rpx_engine::expr::ds::DataSet::new(
        "ds1",
        &["team", "amount"],
        vec![
            vec![Value::Text("A".into()), Value::Int(10)],
            vec![Value::Text("A".into()), Value::Int(20)],
            vec![Value::Text("B".into()), Value::Int(30)],
            vec![Value::Text("B".into()), Value::Int(40)],
        ],
    );
    let g = rpx_engine::calculate(&rd, &[ds]).unwrap();
    assert_eq!(
        column(&g, 3),
        vec![Value::Int(1), Value::Int(2), Value::Int(1), Value::Int(2)]
    );
    assert_eq!(
        column(&g, 4),
        vec![Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)]
    );
}

#[test]
fn explicit_right_hand_head_drives_ordinal_and_copies() {
    let rd = rpx_runtime::reportdef::build(
        &json!({"grid":{"rows":1,"cols":3},"cells":[
        {"row":1,"col":1,"expr":"&C1","left_head":"C1"},
        {"row":1,"col":2,"expr":"C1*10","left_head":"C1"},
        {"row":1,"col":3,"expr":"to(1,3)"}]})
        .to_string(),
    )
    .unwrap();
    let rd = rpx_core::open_bytes(&rpx_core::to_bytes(&rd).unwrap()).unwrap();
    let g = rpx_engine::calculate(&rd, &[]).unwrap();
    assert_eq!(
        column(&g, 1),
        vec![Value::Int(1), Value::Int(2), Value::Int(3)]
    );
    assert_eq!(
        column(&g, 2),
        vec![Value::Int(10), Value::Int(20), Value::Int(30)]
    );
}

#[test]
fn identical_cellsets_follow_education_department_and_report_scopes() {
    let rd = rpx_runtime::reportdef::build(
        &json!({
            "grid":{"rows":5,"cols":4},"cells":[
                {"row":2,"col":1,"expr":"ds1.group(department)"},
                {"row":2,"col":2,"expr":"ds1.group(education)"},
                {"row":2,"col":3,"expr":"ds1.select(person)"},
                {"row":2,"col":4,"expr":"ds1.wage"},
                {"row":3,"col":4,"expr":"sum(D2{})","left_head":"B2"},
                {"row":4,"col":4,"expr":"sum(D2{})","left_head":"A2"},
                {"row":5,"col":4,"expr":"sum(D2{})"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let rows = [
        ("A", "X", 10),
        ("A", "X", 20),
        ("A", "Y", 30),
        ("B", "X", 40),
        ("B", "Y", 50),
    ];
    let ds = rpx_engine::expr::ds::DataSet::new(
        "ds1",
        &["department", "education", "person", "wage"],
        rows.iter()
            .enumerate()
            .map(|(i, (d, e, w))| {
                vec![
                    Value::Text((*d).into()),
                    Value::Text((*e).into()),
                    Value::Int(i as i32),
                    Value::Int(*w),
                ]
            })
            .collect(),
    );
    let g = rpx_engine::calculate(&rd, &[ds]).unwrap();
    let values = |row| {
        g.cells
            .iter()
            .filter(|c| c.origin == (row, 4))
            .map(|c| c.value.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        values(3),
        vec![
            Value::Int(30),
            Value::Int(30),
            Value::Int(40),
            Value::Int(50)
        ]
    );
    assert_eq!(values(4), vec![Value::Int(60), Value::Int(90)]);
    assert_eq!(values(5), vec![Value::Int(150)]);
}

#[test]
fn group_aggregates_use_membership_and_restore_it_after_root_selection() {
    let rd = rpx_runtime::reportdef::build(
        &json!({
            "grid":{"rows":1,"cols":5},"cells":[
                {"row":1,"col":1,"expr":"ds1.group(team)"},
                {"row":1,"col":2,"expr":"ds1.sum(amount*2)"},
                {"row":1,"col":3,"expr":"ds1.count()"},
                {"row":1,"col":4,"expr":"sum(ds1.select@r(amount))"},
                {"row":1,"col":5,"expr":"ds1.sum(amount)"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let ds = rpx_engine::expr::ds::DataSet::new(
        "ds1",
        &["team", "amount"],
        [("A", 10), ("B", 30), ("A", 20), ("B", 40)]
            .map(|(t, v)| vec![Value::Text(t.into()), Value::Int(v)])
            .to_vec(),
    );
    let g = rpx_engine::calculate(&rd, &[ds]).unwrap();
    assert_eq!(column(&g, 2), vec![Value::Int(60), Value::Int(140)]);
    assert_eq!(column(&g, 3), vec![Value::Int(2), Value::Int(2)]);
    assert_eq!(column(&g, 4), vec![Value::Int(100), Value::Int(100)]);
    assert_eq!(column(&g, 5), vec![Value::Int(30), Value::Int(70)]);
}

#[test]
fn nested_filter_restores_the_outer_dollar_anchor() {
    let g = calculate(&[
        "to(1,3)",
        "count(A1[`0]{count(A1[`0]{A1>$A1})>=0 and A1>$A1})",
    ]);
    assert_eq!(
        column(&g, 2),
        vec![Value::Int(2), Value::Int(1), Value::Int(0)]
    );
}

#[test]
fn crossed_group_memberships_intersect_original_rows() {
    let rd = rpx_runtime::reportdef::build(
        &json!({
            "grid":{"rows":2,"cols":2},"cells":[
                {"row":1,"col":2,"expr":"ds1.group(period)","extend_mode":"horizontal"},
                {"row":2,"col":1,"expr":"ds1.group(team)"},
                {"row":2,"col":2,"expr":"ds1.sum(amount)"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let ds = rpx_engine::expr::ds::DataSet::new(
        "ds1",
        &["team", "period", "amount"],
        [
            ("A", 1, 10),
            ("A", 1, 20),
            ("A", 2, 30),
            ("B", 1, 40),
            ("B", 2, 50),
        ]
        .map(|(t, p, v)| vec![Value::Text(t.into()), Value::Int(p), Value::Int(v)])
        .to_vec(),
    );
    let g = rpx_engine::calculate(&rd, &[ds]).unwrap();
    let values: Vec<_> = g
        .cells
        .iter()
        .filter(|c| c.origin == (2, 2))
        .map(|c| c.value.clone())
        .collect();
    assert_eq!(
        values,
        vec![
            Value::Int(30),
            Value::Int(30),
            Value::Int(40),
            Value::Int(50)
        ]
    );
}
