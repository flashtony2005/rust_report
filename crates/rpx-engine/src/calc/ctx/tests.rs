//! [`CalcCtx`] 的单元钉子。
//!
//! [`set_value_binds_bit30`] 守的是本轮最核心的绑定语义："写值"与"标记已求值"
//! 在产品里**不可分**（`setCalc(true)` 藏在 `setValue` 里，`ExtNormalCell.java:1639-1642`）。
//! 若有人把 `CalcCtx::set_value` 拆成"只写值"，通道 4/5 会对同一格重算 ⇒ 值错，
//! 且在当前 12 夹具上**未必**有测试失败（字面量格已由阶段 B 置位，只有"表达式格
//! 被写值"路径受影响）—— 所以要一枚前置钉子。

use crate::arena::CellId;
use crate::calc::ctx::CalcCtx;
use crate::expr::ds::DataSet;
use crate::expr::eval::EvalCtx;
use crate::heads::{HeadCell, HeadState};
use crate::value::Value;

fn ctx_with_one_cell() -> CalcCtx {
    let mut g = HeadState::new();
    g.push(HeadCell::new(0, 0, 16, -91)); // cell00
    let mut c = HeadCell::new(1, 1, 17, -91);
    c.rt.expr = None; // 字面量格形态
    g.push(c);
    CalcCtx::new(
        g,
        vec![DataSet::new("ds1", &["qty"], vec![vec![Value::Int(1)]])],
    )
}

/// `set_value` 必须连 bit30 一起置（`ExtNormalCell.java:1639-1642` 的绑定语义）。
#[test]
fn set_value_binds_bit30() {
    let mut ctx = ctx_with_one_cell();
    let c = CellId(1);
    assert!(ctx.rt(c).unwrap().non_calc(), "前置：初始未算");
    ctx.set_value(c, Value::Int(7));
    let rt = ctx.rt(c).unwrap();
    assert!(
        !rt.non_calc(),
        "写值后必须已算过（setCalc(true) 藏在 setValue 里）"
    );
    assert_eq!(ctx.value_of(c), Value::Int(7));
}

/// `set_calc(false)` 清位**且清值**；`reset_flags` 只清位**不清值**
/// （`setCalc(false)` `:294-297` vs `reset()` `:322-324` —— 两条都有调用点）。
#[test]
fn set_calc_false_clears_value_but_reset_does_not() {
    let mut ctx = ctx_with_one_cell();
    let c = CellId(1);
    ctx.set_value(c, Value::Int(7));

    ctx.reset_flags(c);
    assert!(ctx.rt(c).unwrap().non_calc());
    assert_eq!(
        ctx.value_of(c),
        Value::Int(7),
        "reset() 不清值（rowMerge &= 0x3FFFFFFF）"
    );

    ctx.set_calc(c, false);
    assert!(ctx.rt(c).unwrap().non_calc());
    assert_eq!(
        ctx.value_of(c),
        Value::Null,
        "setCalc(false) 连值一起清（:296）"
    );
}

/// `save_current`/`resume_current` 的指针语义（`ExtCellSet.java:586-596`）：
/// save 快照的是**当时的 `_$21`**，resume 把 `_$21` 拉回快照；`$B2` 读快照。
#[test]
fn save_resume_roundtrip() {
    let mut ctx = ctx_with_one_cell();
    let a = CellId(0);
    let b = CellId(1);
    ctx.set_current(a);
    ctx.save_current(); // _$20 = a
    ctx.set_current(b); // 模拟 sum(A:B) 的 setCurrent
    assert_eq!(ctx.current(), b);
    assert_eq!(ctx.saved_current(), a, "saved 不随 current 变");
    ctx.resume_current();
    assert_eq!(ctx.current(), a, "resume 回到 save 时的格");
}
