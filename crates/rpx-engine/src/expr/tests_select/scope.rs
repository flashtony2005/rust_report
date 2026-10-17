//! 空选择保留行身份，求值退出时恢复调用前的数据集作用域。
use super::{evaluate, parse, root_fake, texts, DataSet, Fake, Value};
use crate::expr::ds::DsCur;

#[test]
fn root_empty_and_single_results() {
    let mut f = root_fake();
    // 过滤无匹配仍记 NullRow，防止整片克隆继承上一片的真实记录。
    let e = parse("ds2.select(item, amount>99999)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
    assert_eq!(f.take_list_note(), None);
    assert_eq!(
        f.take_row_note(),
        Some((0, crate::expr::eval::NULL_ROW_NOTE))
    );

    // 恰好 1 行 ⇒ 单值 + 行旁路（`:177` 的 size==1 分支）
    let e = parse("ds2.select(item, amount==3000)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["年终奖"]);
    assert_eq!(f.take_row_note(), Some((0, 1)));

    // distinct 去空 ⇒ NullRow
    let e = parse("ds2.select(item, amount>99999, , item)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
}

#[test]
fn row_mode_null_row_sentinel_does_not_panic() {
    // 上游 select 过滤未命中把 current 钉成空行哨兵（calc `mirror` 投影
    // `DsCur::Row(NULL_ROW)`）后，本格再求值 DS 函数：**不许 panic**
    // （`selected[i]` 直写会越界），按空行身份返回 Null。
    let mut f = root_fake();
    f.ds_set_row(0, usize::MAX);
    let e = parse("ds2.select(item, amount>=2000)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
    assert_eq!(
        f.take_row_note(),
        Some((0, crate::expr::eval::NULL_ROW_NOTE))
    );
    // sum 的 Row 态路径同样过闸（`dsfn::row_path`）
    let e = parse("ds2.sum(amount)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
    // 空数据集 + 哨兵行也不 panic
    let mut f = Fake::new();
    f.push_ds(DataSet::new("ds2", &["item"], Vec::new()));
    f.ds_set_row(0, usize::MAX);
    let e = parse("ds2.select(item)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
}

#[test]
fn root_pipeline_restores_ds_current() {
    // 产品每条 Group 操作都 save/restore current（`Group.java:239-252` 等）
    // ⇒ 整条 select 求值完，ds 指针必须回到进入时的状态（Root）。
    let mut f = root_fake();
    let e = parse("ds2.select(item, amount>=500, , item)").unwrap();
    let _ = evaluate(&e, &mut f).unwrap();
    assert_eq!(f.ds_current(0), DsCur::Root);
    // Row 态进入也要原样恢复
    f.ds_set_row(0, 2);
    let e = parse("ds2.select(item, emp_id==2)").unwrap();
    let _ = evaluate(&e, &mut f).unwrap();
    assert_eq!(f.ds_current(0), DsCur::Row(2));
}
