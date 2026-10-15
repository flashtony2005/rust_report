//! 第 6 节：跨数据集过滤的**电流语义**（M5-02 验收钉子）。
//!
//! 归属 `super`（tests_select）的第 6 节，因 G4 行数门禁（每 .rs ≤500 行）
//! 拆出。共享夹具（`root_fake` / `texts`）与 M5-01 单测同源 —— 电流语义
//! 必须钉在**同一内存复刻**上，否则对拍结论不可归因。

use crate::arena::CellId;
use crate::expr::eval::evaluate;
use crate::expr::parse;
use crate::value::Value;

use super::{root_fake, texts};

/// `ds2.select(item, emp_id==A3)` 的 `A3` 必须解析到 **current 同 row 的主格
/// 实例**（`resolve.rs::resolve_cell` 走 left/top head 链），不是引擎锚点的
/// 静态值 —— 这正是 T1（`cross_emp_bonus` 从格带）能在**每个主格实例**上
/// 滤出各自奖金的原因。
///
/// # 变异判据（本钉子的存在理由）
///
/// 若实现把 `A3` 短路成引擎锚点值（或永远取第一实例），第二个断言立刻红：
/// current 拨到李娜行时期望 1 项，静态解析会返回张伟的 2 项。
/// 2026-10-15 实测：把 `resolve_cell` 改成 `Some(src)` 直返引擎格后，
/// 本测试第二个断言失败（`["全勤奖", "年终奖"] != ["全勤奖"]`）。
#[test]
fn cross_filter_tracks_master_current_row() {
    let mut f = root_fake(); // ds2 = bonus：emp_id 1,1,2,3,4,4（张伟 2 项 / 李娜 1 项）
                             // 主格 A3：引擎锚点 + 两个实例（张伟 id=1 / 李娜 id=2），18 = 左扩展锚点。
    let a3 = f.put("A3", 18, Value::Int(1));
    let a3_zhang = f.put_inst(a3, 18, Value::Int(1));
    let a3_li = f.put_inst(a3, 18, Value::Int(2));
    f.add_ext(a3, a3_zhang);
    f.add_ext(a3, a3_li);
    f.reg_left(CellId::ROOT, a3);
    f.reg_top(CellId::ROOT, a3);
    // 从格宿主（T1 的 c5 带）：引擎格的左 head 是主格引擎格（模板里从格带
    // 挂在主格带左侧），两个实例的左 head 各指主格**同 row** 实例。
    let host = f.put("", 16, Value::Null);
    f.set_left_head(host, a3);
    let host_zhang = f.put_inst(host, 16, Value::Null);
    let host_li = f.put_inst(host, 16, Value::Null);
    f.set_left_head(host_zhang, a3_zhang);
    f.set_left_head(host_li, a3_li);

    // current = 张伟行 ⇒ A3 解析到 id=1 ⇒ 2 项
    f.set_cur(host_zhang);
    let e = parse("ds2.select(item, emp_id==A3)").unwrap();
    assert_eq!(
        texts(&evaluate(&e, &mut f).unwrap()),
        vec!["全勤奖", "年终奖"]
    );

    // current = 李娜行 ⇒ A3 解析到 id=2 ⇒ 1 项（静态/首例解析在此处红）
    f.set_cur(host_li);
    let e = parse("ds2.select(item, emp_id==A3)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["全勤奖"]);

    // 反向判据：current 回张伟行 ⇒ 又 2 项（排除"只看第二次调用"的假绿）
    f.set_cur(host_zhang);
    let e = parse("ds2.select(item, emp_id==A3)").unwrap();
    assert_eq!(
        texts(&evaluate(&e, &mut f).unwrap()),
        vec!["全勤奖", "年终奖"]
    );
}
