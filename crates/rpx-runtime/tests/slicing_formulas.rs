#![cfg(feature = "legacy-rpx")]
//! 分片公式从 JSON 经二进制往返到真实扩展网格；数据故意乱序、非连续。
use rpx_engine::{arena::CalcGrid, expr::DataSet, value::Value};
use serde_json::{json, Value as Json};

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

fn calculate(doc: Json, data: &[DataSet]) -> rpx_engine::error::Result<CalcGrid> {
    let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    let rd = rpx_core::open_bytes(&rpx_core::to_bytes(&rd).unwrap()).unwrap();
    rpx_engine::calculate(&rd, data)
}

fn values(g: &CalcGrid, origin: (i32, i32)) -> Vec<Value> {
    g.cells
        .iter()
        .filter(|c| c.origin == origin && c.is_emitted())
        .map(|c| c.value.clone())
        .collect()
}

fn scalars(expressions: &[&str]) -> rpx_engine::error::Result<CalcGrid> {
    calculate(
        json!({"grid":{"rows":1,"cols":expressions.len()},"cells":expressions.iter()
        .enumerate().map(|(i,e)|json!({"row":1,"col":i+1,"expr":e,"extend_mode":"none"}))
        .collect::<Vec<_>>()}),
        &[],
    )
}

#[test]
fn membership_uses_existing_equality_for_numbers_null_and_text() {
    let expressions = [
        "1 in list(1.0,2)",
        "null in [null,1]",
        "null in []",
        "'01' in [1,2]",
        "'华南' in ('华南','西南')",
        "3 in (1,2)",
        "1 in 1.0",
        "1 in (1)+1",
        "false in list(true,false)",
        "list(1,2) in list(2.0,1,3)",
        "list(1,4) in list(1,2,3)",
        "[] in []",
        "list(1.0) in 1",
    ];
    let g = scalars(&expressions).unwrap();
    for (i, expected) in [
        true, true, false, false, true, false, true, false, true, true, false, true, true,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            values(&g, (1, i as i32 + 1)),
            vec![Value::Bool(expected)],
            "{}",
            expressions[i]
        );
    }
}

#[test]
fn date_components_validate_gregorian_boundaries_and_timestamps() {
    for (date, y, m) in [
        ("0001-01-01", 1, 1),
        ("9999-12-31", 9999, 12),
        ("2000-02-29", 2000, 2),
        ("2024-02-29 23:59:59", 2024, 2),
        ("1900-03-01", 1900, 3),
        ("2025-01-01T00:00:00.123Z", 2025, 1),
        ("2024-12-31T23:59:59+14:00", 2024, 12),
        ("2024-02-29 10:20:30:422", 2024, 2),
    ] {
        let g = scalars(&[
            &format!("year('{date}')"),
            &format!("month('{date}')"),
            &format!("month@y('{date}')"),
        ])
        .unwrap();
        assert_eq!(values(&g, (1, 1)), vec![Value::Int(y)]);
        assert_eq!(values(&g, (1, 2)), vec![Value::Int(m)]);
        assert_eq!(values(&g, (1, 3)), vec![Value::Int(y * 100 + m)]);
    }
    let g = scalars(&["year(null)", "month(null)"]).unwrap();
    assert!(values(&g, (1, 1)).is_empty());
    assert!(values(&g, (1, 2)).is_empty());
}

#[test]
fn invalid_dates_and_function_shapes_produce_diagnostics() {
    for date in [
        "1900-02-29",
        "2100-02-29",
        "2023-02-29",
        "2024-04-31",
        "0000-01-01",
        "2024-00-01",
        "2024-13-01",
        "2024-01-00",
        "2024-01-32",
        "2024-1-01",
        "",
        "日期错误",
        "2024-01-01junk",
        "2024-01-01 24:00:00",
        "2024-01-01T00:60:00",
        "2024-01-01T00:00:60",
        "2024-01-01T00:00:00.",
        "2024-01-01T00:00:00+14:01",
    ] {
        for function in ["year", "month"] {
            let err = scalars(&[&format!("{function}('{date}')")])
                .unwrap_err()
                .to_string();
            assert!(err.contains(function) && err.contains("非法日期"), "{err}");
        }
    }
    for expr in [
        "year()",
        "month(1)",
        "year('2024-01-01',2)",
        "month@x('2024-01-01')",
    ] {
        assert!(scalars(&[expr]).is_err(), "{expr}");
    }
}

#[test]
fn tutorial_year_month_group_sort_binds_bare_fields_and_preserves_original_rows() {
    let g = calculate(
        json!({"grid":{"rows":1,"cols":4},"cells":[
            {"row":1,"col":1,"expr":"sales.group(year(ordered_at);year(ordered_at):1)"},
            {"row":1,"col":2,"expr":"sales.group(month(ordered_at);month(ordered_at):1)"},
            {"row":1,"col":3,"expr":"sales.sum(amount,amount!=null)"},
            {"row":1,"col":4,"expr":"sales.count()"}
        ]}),
        &[DataSet::new(
            "sales",
            &["ordered_at", "amount"],
            vec![
                vec![text("2025-03-01"), Value::Int(80)],
                vec![text("2024-02-29"), Value::Int(20)],
                vec![text("2025-01-01"), Value::Int(40)],
                vec![text("2024-01-31"), Value::Int(10)],
                vec![text("2024-02-01"), Value::Int(5)],
                vec![text("2025-03-31"), Value::Null],
            ],
        )],
    )
    .unwrap();
    assert_eq!(
        values(&g, (1, 2)),
        vec![Value::Int(1), Value::Int(2), Value::Int(1), Value::Int(3)]
    );
    assert_eq!(
        values(&g, (1, 1)),
        vec![
            Value::Int(2024),
            Value::Int(2024),
            Value::Int(2025),
            Value::Int(2025)
        ]
    );
    assert_eq!(
        values(&g, (1, 3)),
        vec![
            Value::Int(10),
            Value::Int(25),
            Value::Int(40),
            Value::Int(80)
        ]
    );
    assert_eq!(
        values(&g, (1, 4)),
        vec![Value::Int(1), Value::Int(2), Value::Int(1), Value::Int(2)]
    );
}

#[test]
fn group_filter_excludes_rows_before_evaluating_date_keys() {
    let g = calculate(
        json!({"grid":{"rows":1,"cols":2},"cells":[
            {"row":1,"col":1,"expr":"ds.group(year(date),keep;year(date):1)"},
            {"row":1,"col":2,"expr":"ds.sum(amount)"}
        ]}),
        &[DataSet::new(
            "ds",
            &["date", "keep", "amount"],
            vec![
                vec![text("bad"), Value::Bool(false), Value::Int(999)],
                vec![text("2024-02-29"), Value::Bool(true), Value::Int(10)],
                vec![Value::Null, Value::Bool(false), Value::Int(888)],
            ],
        )],
    )
    .unwrap();
    assert_eq!(values(&g, (1, 1)), vec![Value::Int(2024)]);
    assert_eq!(values(&g, (1, 2)), vec![Value::Int(10)]);
}

fn category_data() -> DataSet {
    DataSet::new(
        "products",
        &["category", "product_id", "priority"],
        vec![
            vec![text("B"), Value::Int(21), Value::Int(2)],
            vec![text("A"), Value::Int(11), Value::Int(1)],
            vec![Value::Null, Value::Int(99), Value::Int(0)],
            vec![text("B"), Value::Int(20), Value::Int(3)],
            vec![text("A"), Value::Int(10), Value::Int(3)],
            vec![text("A"), Value::Int(12), Value::Int(3)],
            vec![text("C"), Value::Int(30), Value::Int(0)],
        ],
    )
}

#[test]
fn group_reuses_multikey_sort_with_stable_ties_and_noncontiguous_members() {
    let g = calculate(json!({"grid":{"rows":1,"cols":3},"cells":[
        {"row":1,"col":1,"expr":"products.group(category,category!=null;category:1,priority:-1)"},
        {"row":1,"col":2,"expr":"products.select(product_id)"},
        {"row":1,"col":3,"expr":"products.priority"}
    ]}), &[category_data()]).unwrap();
    assert_eq!(values(&g, (1, 2)), [10, 12, 11, 20, 21, 30].map(Value::Int));
    assert_eq!(values(&g, (1, 3)), [3, 3, 1, 3, 2, 0].map(Value::Int));
}

#[test]
fn group_shorthand_sorts_both_directions_and_nulls_keep_members() {
    for (expr, expected) in [
        (
            "products.group(category:1)",
            vec![99, 11, 10, 12, 21, 20, 30],
        ),
        (
            "products.group(category:-1)",
            vec![30, 21, 20, 11, 10, 12, 99],
        ),
        (
            "products.group(category,category!=null;category:-1)",
            vec![30, 21, 20, 11, 10, 12],
        ),
    ] {
        let g = calculate(
            json!({"grid":{"rows":1,"cols":2},"cells":[
                {"row":1,"col":1,"expr":expr},
                {"row":1,"col":2,"expr":"products.select(product_id)"}
            ]}),
            &[category_data()],
        )
        .unwrap();
        assert_eq!(
            values(&g, (1, 2)),
            expected.into_iter().map(Value::Int).collect::<Vec<_>>(),
            "{expr}"
        );
    }
}

#[test]
fn employee_by_category_sum_keeps_inner_dataset_scope_and_filters_by_cell() {
    let sales = DataSet::new(
        "sales",
        &["employee_id", "product_id", "qty", "price"],
        [
            (2, 10, 4, 10),
            (1, 21, 2, 10),
            (1, 10, 3, 10),
            (2, 20, 5, 10),
            (1, 20, 1, 10),
            (2, 11, 6, 10),
        ]
        .map(|(e, p, q, v)| vec![Value::Int(e), Value::Int(p), Value::Int(q), Value::Int(v)])
        .to_vec(),
    );
    let staff = DataSet::new(
        "staff",
        &["employee_id"],
        vec![vec![Value::Int(1)], vec![Value::Int(2)]],
    );
    let g = calculate(json!({"grid":{"rows":8,"cols":4},"cells":[
        {"row":7,"col":3,"expr":"products.group(category,category!=null;category:1)","extend_mode":"horizontal","left_head":"`0","top_head":"`0"},
        {"row":8,"col":2,"expr":"staff.select(employee_id)","left_head":"`0","top_head":"`0"},
        {"row":8,"col":3,"expr":"sales.sum(qty*price,employee_id==B8 and product_id in products.select(product_id))","left_head":"B8","top_head":"C7","extend_mode":"none"},
        {"row":8,"col":4,"expr":"sales.sum(qty*price)","left_head":"B8","top_head":"`0","extend_mode":"none"}
    ]}), &[sales,staff,category_data()]).unwrap();
    assert_eq!(
        values(&g, (8, 3)),
        vec![
            Value::Int(30),
            Value::Int(30),
            Value::Double(0.0),
            Value::Int(100),
            Value::Int(50),
            Value::Double(0.0)
        ]
    );
    assert_eq!(values(&g, (8, 4)), vec![Value::Int(210); 2]);
}

#[test]
fn filtered_sum_handles_null_values_empty_groups_and_row_scope() {
    let ds = DataSet::new(
        "ds",
        &["amount", "keep"],
        vec![
            vec![Value::Int(10), Value::Bool(true)],
            vec![Value::Null, Value::Bool(true)],
            vec![text("invalid"), Value::Bool(false)],
            vec![Value::Int(20), Value::Bool(true)],
        ],
    );
    let g = calculate(
        json!({"grid":{"rows":1,"cols":3},"cells":[
            {"row":1,"col":1,"expr":"ds.sum(amount,keep)"},
            {"row":1,"col":2,"expr":"ds.sum(amount,false)"},
            {"row":1,"col":3,"expr":"ds.count()"}
        ]}),
        &[ds],
    )
    .unwrap();
    assert_eq!(values(&g, (1, 1)), vec![Value::Int(30)]);
    assert_eq!(values(&g, (1, 2)), vec![Value::Double(0.0)]);
    assert_eq!(values(&g, (1, 3)), vec![Value::Int(4)]);
}

#[test]
fn empty_order_details_do_not_reuse_previous_piece_rows_or_amounts() {
    let orders = DataSet::new(
        "orders",
        &["order_id", "shipping"],
        [(107, 29), (108, 31), (109, 37), (110, 41), (111, 43)]
            .map(|(id, s)| vec![Value::Int(id), Value::Int(s)])
            .to_vec(),
    );
    let lines = DataSet::new(
        "lines",
        &["order_id", "product_id", "qty", "price"],
        [
            (108, 21, 8, 100),
            (110, 10, 2, 10),
            (107, 11, 7, 100),
            (110, 11, 3, 10),
        ]
        .map(|(o, p, q, v)| vec![Value::Int(o), Value::Int(p), Value::Int(q), Value::Int(v)])
        .to_vec(),
    );
    let g = calculate(json!({"grid":{"rows":4,"cols":9},"cells":[
        {"row":1,"col":2,"expr":"orders.select(order_id)","left_head":"`0","top_head":"`0"},
        {"row":2,"col":2,"expr":"lines.select(product_id:1,order_id==B1)","left_head":"B1","top_head":"`0"},
        {"row":2,"col":3,"expr":"lines.qty","left_head":"B2","top_head":"`0"},
        {"row":2,"col":4,"expr":"lines.price","left_head":"B2","top_head":"`0"},
        {"row":2,"col":5,"expr":"C2*D2","left_head":"B2","top_head":"`0"},
        {"row":3,"col":5,"expr":"sum(E2{})","left_head":"B1","top_head":"`0"},
        {"row":3,"col":7,"expr":"orders.shipping","left_head":"B1","top_head":"`0"},
        {"row":3,"col":9,"expr":"E3+G3","left_head":"B1","top_head":"`0"},
        {"row":4,"col":5,"expr":"sum(E3{})","left_head":"`0","top_head":"`0"}
    ]}), &[orders,lines]).unwrap();
    assert_eq!(
        values(&g, (3, 5)),
        vec![
            Value::Int(700),
            Value::Int(800),
            Value::Double(0.0),
            Value::Int(50),
            Value::Double(0.0)
        ]
    );
    assert_eq!(
        values(&g, (3, 9)),
        vec![
            Value::Int(729),
            Value::Int(831),
            Value::Double(37.0),
            Value::Int(91),
            Value::Double(43.0)
        ]
    );
    assert_eq!(values(&g, (4, 5)), vec![Value::Double(1550.0)]);
}
