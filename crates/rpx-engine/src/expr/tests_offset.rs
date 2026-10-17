//! `ds[i].列`（DSOffset + DSMember）求值单测 —— 探针 `ProbeDsOffset` O1–O9 实证
//! （2026-10-16，scratch 在 `re_work/tmp_probe/dsoffset/`，数据集 5 行：
//! North/100, East/200, South/150, West/300, Mid/250）。
//!
//! 判据分四组：
//! 1. **Root 态**：`[1]`/`[2]`/`[5]` 读对应行；`[9]`/`[0]`/`[-1]` 越界 ⇒ Null（不报错）；
//! 2. **Row 态**：偏移被忽略、读当前行（O6 纵带实测每行 = 当前行值）；
//! 3. **下标校验**：非整数报 paramTypeError（产品消息键，`ds1["a"]` O8）；
//!    `ds1[1+1]` 非叶子运算接受（O9 = 200）；
//! 4. **语法边界**：裸 `ds1[2]` parse 期显式拒绝（产品返回 DataSet 对象，O7）；
//!    `A1[2]` 仍走 Move/Offset 路径（eval 报 UnsupportedExpr，行为不变）。

use crate::expr::ds::DataSet;
use crate::expr::eval::evaluate;
use crate::expr::parse;
use crate::value::Value;

use super::tests_eval::Fake;

/// 探针同款 5 行数据集（region/qty）。
fn probe_ds() -> DataSet {
    DataSet::new(
        "ds1",
        &["region", "qty"],
        vec![
            vec![Value::Text("North".into()), Value::Int(100)],
            vec![Value::Text("East".into()), Value::Int(200)],
            vec![Value::Text("South".into()), Value::Int(150)],
            vec![Value::Text("West".into()), Value::Int(300)],
            vec![Value::Text("Mid".into()), Value::Int(250)],
        ],
    )
}

fn root_eval(src: &str) -> Result<Value, crate::error::EngineError> {
    let mut f = Fake::new();
    f.push_ds(probe_ds());
    evaluate(&parse(src)?, &mut f)
}

#[test]
fn root_state_reads_the_offset_row() {
    // O1/O2/O3：Root 态按 i 取行（产品 getCurrentData 的 Group 分支）
    assert!(matches!(root_eval("ds1[2].qty"), Ok(Value::Int(200))));
    assert!(matches!(root_eval("ds1[1].qty"), Ok(Value::Int(100))));
    assert!(matches!(root_eval("ds1[5].qty"), Ok(Value::Int(250))));
    // 列也可以
    assert!(matches!(root_eval("ds1[3].region"), Ok(Value::Text(ref s)) if s == "South"));
}

#[test]
fn root_state_out_of_range_is_null_not_error() {
    // O4/O5/O6：越界（`>rowCount` / `<1`）⇒ Null，产品不报错
    assert!(matches!(root_eval("ds1[9].qty"), Ok(Value::Null)));
    assert!(matches!(root_eval("ds1[0].qty"), Ok(Value::Null)));
    assert!(matches!(root_eval("ds1[-1].qty"), Ok(Value::Null)));
}

#[test]
fn row_state_ignores_the_offset() {
    // O6：Row 态读当前行（产品 Row 分支不看 `_$5`）
    let mut f = Fake::new();
    f.push_ds(probe_ds());
    let e = parse("ds1[2].qty").unwrap();
    for (row, want) in [(0usize, 100i32), (1, 200), (2, 150), (3, 300), (4, 250)] {
        f.ds_set_row(0, row);
        assert!(
            matches!(evaluate(&e, &mut f), Ok(Value::Int(v)) if v == want),
            "Row 态第 {row} 行应读当前行 {want}"
        );
    }
}

#[test]
fn non_integer_offset_is_a_param_type_error() {
    // O8：`ds1["a"].qty` —— 产品 `DSOffset.java:22-26` 判 instanceof Integer
    let err = root_eval("ds1[\"a\"].qty").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("paramTypeError"),
        "错误消息须点名产品消息键，实得：{msg}"
    );
    // Double 同样拒（产品判 Integer 不是 Number）
    let err2 = root_eval("ds1[1.5].qty").unwrap_err();
    assert!(err2.to_string().contains("paramTypeError"), "{err2}");
}

#[test]
fn a_computed_offset_expression_is_accepted() {
    // O9：`ds1[1+1].qty` = 200（非叶子运算产品也接受）
    assert!(matches!(root_eval("ds1[1+1].qty"), Ok(Value::Int(200))));
}

#[test]
fn bare_offset_without_a_column_is_refused_at_eval_time() {
    // O7：产品返回 DataSet 对象（无实用对拍形态）。M7-8 起「格号 + `[...]`」统一
    // 走 `Expr::Coord`（坐标），所以裸 `ds1[2]` 在求值期报 **UnknownCell{"ds1"}**
    // —— 显式失败，不猜（`ds1` 不是引擎矩阵里的格号）。
    let e = parse("ds1[2]").unwrap();
    assert!(matches!(e, crate::expr::Expr::Coord { .. }), "应为坐标节点");
    let mut f = Fake::new();
    f.push_ds(probe_ds());
    let err = evaluate(&e, &mut f).unwrap_err();
    assert!(
        err.to_string().contains("ds1"),
        "应点名 ds1 不是格号，实得：{err}"
    );
}

#[test]
fn cell_id_with_bracket_is_a_coordinate_against_the_engine_matrix() {
    // M7-8 行为变更：`A1[2]` 不再落 Move/Offset 兜底，而是**坐标**。
    // 假网格里没有 A1 ⇒ 求值报 UnknownCell（显式失败；有真网格时按层次坐标解析）。
    let e = parse("A1[2]").unwrap();
    assert!(matches!(e, crate::expr::Expr::Coord { .. }), "应为坐标节点");
    let mut f = Fake::new();
    let err = evaluate(&e, &mut f).unwrap_err();
    assert!(err.to_string().contains("A1"), "{err}");
}

/// `[...]` 挂在**非格号**左操作数上时仍走 `Expr::Index` 兜底（`Move`/`Offset` 未实现）。
#[test]
fn parenthesised_postfix_keeps_the_move_stub() {
    let e = parse("(1+2)[1]").unwrap();
    assert!(
        matches!(e, crate::expr::Expr::Index { .. }),
        "非格号左操作数 ⇒ Move/Offset 兜底形态"
    );
    let mut f = Fake::new();
    let err = evaluate(&e, &mut f).unwrap_err();
    assert!(err.to_string().contains("Move/Offset"), "{err}");
}

#[test]
fn missing_dataset_or_column_still_errors_loudly() {
    let mut f = Fake::new();
    f.push_ds(probe_ds());
    // 数据集缺失
    let e = evaluate(&parse("ds9[1].qty").unwrap(), &mut f).unwrap_err();
    assert!(e.to_string().contains("ds9"), "{e}");
    // 列缺失
    let e2 = evaluate(&parse("ds1[1].nope").unwrap(), &mut f).unwrap_err();
    assert!(e2.to_string().contains("nope"), "{e2}");
}
