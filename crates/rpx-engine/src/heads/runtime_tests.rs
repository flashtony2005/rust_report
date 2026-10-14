//! [`CellRuntime`]（运行态）的钉子测试。
//!
//! 钉子 [`cloning_copies_runtime_state_instead_of_sharing_it`] 守的是
//! `runtime.rs` 模块文档里那条**值关键的结论**：
//!
//! > `ExtNormalCell.clone:136` 的 `&= Integer.MAX_VALUE` 抹的是 **bit31**
//! > （"已被要求求值"），**bit30（"已求值"）被保留** ⇒ 扩展克隆**继承"已求值"**，
//! > 不会被通道 4/5 再算一遍。
//!
//! 如果有人把 `clone_cell` 改成"克隆后把 `calculated` 清干净"（看似更卫生、
//! 实则让每个克隆被重新求值 ⇒ 值翻倍/串行），本测试必须红。当前 12 个夹具上
//! 该改动**不会**让任何既有测试失败 —— 这正是钉子存在的理由。

use crate::arena::CellId;
use crate::calc::ds_current::DsInstance;
use crate::heads::{HeadCell, HeadState};
use crate::value::Value;

/// 克隆是**逐字段拷贝**运行态，而不是共享/清空（`ExtNormalCell.java:133-148` +
/// `runtime.rs` 模块文档）。
#[test]
fn cloning_copies_runtime_state_instead_of_sharing_it() {
    let mut g = HeadState::new();
    // 锚点：已求值（bit30）+ 已被要求过（bit31）+ 有 dispValue 覆盖 + 有实例 + 有状态。
    let mut anchor = HeadCell::new(1, 1, 18, -91);
    anchor.value = Value::Int(100);
    anchor.disp_override = Some("覆盖".into());
    anchor.rt.calculated = true; // bit30
    anchor.rt.demanded = true; // bit31
    anchor.rt.state = 4;
    anchor.rt.instance = Some(DsInstance::new(0, 2));
    let a = g.push(anchor);
    let c = g.clone_cell(a);

    // bit30 **保留**：克隆继承"已求值"（值关键，别改）。
    assert!(
        g.get(c).unwrap().rt.calculated,
        "clone 必须继承 bit30（clone:136 只抹 bit31）"
    );
    // bit31 清零：克隆不继承"已被要求求值"。
    assert!(
        !g.get(c).unwrap().rt.demanded,
        "clone 必须抹 bit31（clone:136）"
    );
    // propSetMap = null（clone:135）⇒ 覆盖项不继承；模板声明继承。
    assert_eq!(
        g.get(c).unwrap().disp_override,
        None,
        "clone 不继承 dispValue 覆盖"
    );
    // `_$4 = null`（ExtExtendCell.clone:52）。
    assert_eq!(g.get(c).unwrap().rt.instance, None, "clone 不继承 _$4");
    // 其余运行态字段按值拷贝（Java 的 engine 是引用共享，本 crate 有意偏离、
    // 无夹具可辨 —— 见 runtime.rs 模块文档）。
    assert_eq!(g.get(c).unwrap().rt.state, 4);
    // 锚点自身不被克隆动作波及。
    let a2 = g.get(a).unwrap();
    assert!(a2.rt.calculated && a2.rt.demanded);
    assert_eq!(a2.disp_override.as_deref(), Some("覆盖"));
    assert_eq!(a2.rt.instance, Some(DsInstance::new(0, 2)));
    // 克隆拿到的是新 id，不是锚点本体（`is_original` 语义的前提）。
    assert_ne!(a, c);
    assert_ne!(c, CellId::NONE);
}
