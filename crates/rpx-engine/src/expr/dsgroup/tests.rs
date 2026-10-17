//! 求值失败同样恢复外层 Row/Group/Root，避免后续分片读取到内层记录。
use crate::expr::{evaluate, parse, tests_eval::Fake, DataSet, DsCur, EvalCtx};
use crate::value::Value;

fn context() -> Fake {
    let mut ctx = Fake::new();
    ctx.push_ds(DataSet::new(
        "ds",
        &["key", "amount", "date"],
        vec![
            vec![
                Value::Int(2),
                Value::Int(10),
                Value::Text("2024-02-29".into()),
            ],
            vec![Value::Null, Value::Int(20), Value::Text("bad-date".into())],
            vec![
                Value::Int(1),
                Value::Int(30),
                Value::Text("2025-01-01".into()),
            ],
        ],
    ));
    ctx
}

#[test]
fn root_group_and_row_restore_on_success_and_failure() {
    let mut ctx = context();
    let group = DsCur::Group(ctx.ds("ds").unwrap().intern_group(vec![2, 0]));
    for saved in [DsCur::Root, group, DsCur::Row(2)] {
        for src in [
            "ds.sum@r(amount,key!=null)",
            "ds.group@r(key;key:1)",
            "ds.select@r(key;key:1)",
            "ds.sum(amount,key!=null)",
            "ds.group(key;key:-1)",
        ] {
            ctx.ds("ds").unwrap().set_current(saved);
            evaluate(&parse(src).unwrap(), &mut ctx).unwrap();
            assert_eq!(ctx.ds("ds").unwrap().current, saved, "{src}");
        }
        for src in [
            "ds.sum@r(year(date),true)",
            "ds.sum@r(amount,year(date)==2024)",
            "ds.group@r(year(date);key:1)",
            "ds.group@r(key;year(date):1)",
            "ds.group@r(key,year(date)==2024;key:1)",
            "ds.select@r(key;year(date):1)",
            "ds.select@r(year(date))",
            "ds.select@r(key,year(date)==2024)",
        ] {
            ctx.ds("ds").unwrap().set_current(saved);
            assert!(evaluate(&parse(src).unwrap(), &mut ctx).is_err(), "{src}");
            assert_eq!(ctx.ds("ds").unwrap().current, saved, "{src}");
        }
    }
}

#[test]
fn noncontiguous_subgroup_sum_and_nested_root_query_restore_scope() {
    let mut ctx = context();
    let group = DsCur::Group(ctx.ds("ds").unwrap().intern_group(vec![2, 0]));
    ctx.ds("ds").unwrap().set_current(group);
    for (src, expected) in [
        ("ds.sum(amount,key!=null)", Value::Int(40)),
        (
            "ds.sum(amount,amount==ds.sum@r(amount,key==1))",
            Value::Int(30),
        ),
        ("ds.sum(amount,key in ds.select@r(key))", Value::Int(40)),
        ("ds.sum(amount,false)", Value::Double(0.0)),
    ] {
        assert_eq!(
            evaluate(&parse(src).unwrap(), &mut ctx).unwrap(),
            expected,
            "{src}"
        );
        assert_eq!(ctx.ds("ds").unwrap().current, group);
    }
}

#[test]
fn row_sum_filter_and_empty_row_preserve_row_identity() {
    let mut ctx = context();
    for (row, src, expected) in [
        (0, "ds.sum(amount,key==2)", Value::Int(10)),
        (0, "ds.sum(amount,key==1)", Value::Null),
        (usize::MAX, "ds.sum(amount,true)", Value::Null),
    ] {
        ctx.ds("ds").unwrap().set_current(DsCur::Row(row));
        assert_eq!(evaluate(&parse(src).unwrap(), &mut ctx).unwrap(), expected);
        assert_eq!(ctx.ds("ds").unwrap().current, DsCur::Row(row));
    }
}

#[test]
fn malformed_group_arguments_diagnose_instead_of_silently_ignoring_parts() {
    let mut ctx = context();
    for src in [
        "ds.group()",
        "ds.group(key,true,1)",
        "ds.group(key:1;key:1)",
        "ds.group(key;key:1;ds.sum(amount))",
        "ds.sum(amount,true,1)",
    ] {
        assert!(evaluate(&parse(src).unwrap(), &mut ctx).is_err(), "{src}");
        assert_eq!(ctx.ds("ds").unwrap().current, DsCur::Root);
    }
}

#[test]
fn date_null_is_a_null_value_before_emission() {
    let mut ctx = context();
    for src in ["year(null)", "month(null)"] {
        assert_eq!(
            evaluate(&parse(src).unwrap(), &mut ctx).unwrap(),
            Value::Null
        );
    }
}
