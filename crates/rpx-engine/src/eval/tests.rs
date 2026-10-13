//! 求值调度器的脚本化假网格单测（§12.1 第 7 步，附件 K.1）。
//!
//! 每条用例对准 K.1 的一处移植细节，出处以 `ExtCellSet.java:NNN` 标注。
//! 假实现本体与脚本类型在 [`harness`]，边界型用例（真错误穿透 / 中途插入的行列）
//! 在 [`sched_edges`] —— 两处都是**门禁行数**拆分，不是功能分层。

use std::collections::VecDeque;

use super::sched::{channel4, channel5, channel6, run_channels};
use super::{state, Flow};
use crate::arena::CellId;
use crate::error::EngineError;

mod harness;
mod sched_edges;

use harness::{FakeGrid, Step};

/// 通道 4 先扫 19（横向）一趟、再扫 18（纵向）一趟（`:1301-1305`）。
#[test]
fn channel4_visits_19_before_18() {
    let mut g = FakeGrid::new(3, 3);
    let a = g.put(1, 1, 19, true);
    let b = g.put(2, 1, 18, true);
    assert!(channel4(&mut g, &mut Vec::new()).is_ok());
    assert_eq!(g.prep, vec![a, b]);
    assert_eq!(g.engine, vec![state::FIRST]);
}

/// 槽位同一性：格坐在 (1,1) 却自称在 (2,2) ⇒ 跳过（`:1308`）。
#[test]
fn channel4_skips_slot_mismatch() {
    let mut g = FakeGrid::new(3, 3);
    let a = g.cell(2, 2, 19, true);
    g.at.insert((1, 1), a);
    assert!(channel4(&mut g, &mut Vec::new()).is_ok());
    assert!(g.prep.is_empty());
}

/// state 4 换格 chase：A 挂起要 B → 算 B → 回卷继续算 A（`:1322-1328`）。
///
/// B 同时坐在 (2,1) 槽位：A 的 chase 结束后，外层扫描会**独立**再算它一次
/// —— 这正是「chase 轨迹」与「扫描轨迹」叠加的验证点。
#[test]
fn channel4_chase_and_unwind() {
    let mut g = FakeGrid::new(3, 3);
    let a = g.put(1, 1, 19, true);
    let b = g.put(2, 1, 19, true);
    g.script(a, vec![Step::To(b, state::FIRST), Step::Ok]);
    g.script(b, vec![Step::Ok]);
    let mut st = Vec::new();
    assert!(channel4(&mut g, &mut st).is_ok());
    assert!(st.is_empty(), "回卷完成后依赖栈应清空");
    assert_eq!(g.prep, vec![a, b, a, b], "chase 轨迹 [a,b,a] + 扫描轨迹 [b]");
}

/// state 6：入队的是**当前格**、reset 的是**挂起方**、st **不**排空（`:1398-1401`）。
///
/// 挂起方 b 用 `cell()`（不落槽位）：否则外层扫描扫到它会回卷排空 st，
/// 观测点被冲掉。
#[test]
fn channel5_state6_pushes_current_cell_not_suspended() {
    let mut g = FakeGrid::new(3, 3);
    let a = g.put(1, 1, 17, true);
    let b = g.cell(1, 2, 17, true);
    let c = g.cell(9, 9, 17, true); // state 6 的挂起方（脚本引用）
    g.script(a, vec![Step::To(b, state::FIRST)]); // 先制造非空 st
    g.script(b, vec![Step::To(c, state::SPECIAL)]);
    let mut st = Vec::new();
    let mut special = VecDeque::new();
    assert!(channel5(&mut g, &mut special, &mut st).is_ok());
    assert_eq!(special, VecDeque::from([b]), "⛔ 入队的是当前格 b，不是挂起方 c");
    assert_eq!(st, vec![a], "⛔ st 未被排空");
    assert_eq!(g.resets, vec![c], "reset 的是挂起方 c");
}

/// state 8 级联失效：本体格（含排空栈里的）标 8 入失效队列（`:1403-1417`）。
#[test]
fn channel5_state8_cascade_marks_originals() {
    let mut g = FakeGrid::new(3, 3);
    let a = g.put(1, 1, 17, true);
    let b = g.put(1, 2, 17, true);
    let c = g.cell(9, 9, 17, true);
    g.script(a, vec![Step::To(b, state::FIRST)]);
    g.script(b, vec![Step::To(c, state::THIRD)]);
    let mut st = Vec::new();
    let mut special = VecDeque::new();
    assert!(channel5(&mut g, &mut special, &mut st).is_ok());
    assert!(st.is_empty(), "级联后整个依赖栈被排空");
    assert!(special.is_empty());
    assert_eq!(g.failq, vec![c, a], "本体格 c、a 依次入失效队列");
    assert_eq!(g.cal8, vec![b, c, a], "当前格 b + 挂起方 c + 栈内 a 全标 8");
}

/// state 8 只标**本体格**：克隆实例（`getSource()!=this`）只 reset 不入队。
#[test]
fn channel5_state8_skips_clones() {
    let mut g = FakeGrid::new(3, 3);
    let a = g.put(1, 1, 17, false); // 克隆：扩展产生的新实例
    let b = g.put(1, 2, 17, true);
    let c = g.cell(9, 9, 17, true);
    g.script(a, vec![Step::To(b, state::FIRST)]);
    g.script(b, vec![Step::To(c, state::THIRD)]);
    let mut st = Vec::new();
    let mut special = VecDeque::new();
    assert!(channel5(&mut g, &mut special, &mut st).is_ok());
    assert_eq!(g.resets, vec![c, a], "克隆 a 仍被 reset");
    assert_eq!(g.failq, vec![c], "⛔ 克隆 a 不得入失效队列");
    assert_eq!(g.cal8, vec![b, c], "⛔ 克隆 a 不得标 8");
}

/// 通道 6 正常排空：队列逐个求值至空。
#[test]
fn channel6_drains_queue() {
    let mut g = FakeGrid::new(2, 2);
    let x = g.cell(1, 1, 17, true);
    let y = g.cell(2, 1, 17, true);
    let mut special = VecDeque::from([x, y]);
    assert!(channel6(&mut g, &mut special, &mut Vec::new(), 10).is_ok());
    assert!(special.is_empty());
    assert_eq!(g.prep, vec![x, y]);
    assert_eq!(g.engine, vec![state::SPECIAL]);
}

/// 通道 6 活锁兜底：恒抛 6 的格反复重入，超限报 `SpecialQueueLivelock`。
#[test]
fn channel6_livelock_bounded() {
    let mut g = FakeGrid::new(2, 2);
    let x = g.cell(1, 1, 17, true);
    g.cells[x.0 as usize].loop6 = true;
    let mut special = VecDeque::from([x]);
    let err = channel6(&mut g, &mut special, &mut Vec::new(), 3).unwrap_err();
    assert!(matches!(
        err,
        Flow::Fatal(EngineError::SpecialQueueLivelock { requeues: 4 })
    ));
}

/// 中断检查：通道立即以 Fatal 退出，run_channels 原样透传（`_$8()`:233-237）。
#[test]
fn interruption_short_circuits() {
    let mut g = FakeGrid::new(2, 2);
    g.interrupted = true;
    assert!(matches!(
        channel4(&mut g, &mut Vec::new()),
        Err(Flow::Fatal(EngineError::Interrupted))
    ));
    assert!(matches!(run_channels(&mut g), Err(EngineError::Interrupted)));
}

/// 通道 4 只消化 state 4：state 5 上抛 ⇒ run_channels 报断言式错误。
#[test]
fn unhandled_state_maps_to_assertion_error() {
    let mut g = FakeGrid::new(2, 2);
    let a = g.put(1, 1, 19, true);
    g.script(a, vec![Step::To(CellId::NONE, state::SECOND)]);
    let err = run_channels(&mut g).unwrap_err();
    assert!(
        matches!(&err, EngineError::UnsupportedExpr { expr } if expr.contains("5")),
        "实际错误：{err:?}"
    );
}

/// state 1（CIRCLE）在通道边界映射为 `CircularRef`（M2-07 对外错误形状）。
#[test]
fn circle_state_maps_to_circular_ref() {
    let mut g = FakeGrid::new(2, 2);
    let a = g.put(1, 1, 17, true);
    g.script(a, vec![Step::To(CellId::NONE, state::CIRCLE)]);
    assert!(matches!(
        run_channels(&mut g),
        Err(EngineError::CircularRef { .. })
    ));
}

/// 统一入口的通道切换顺序：4 → 5 → 6 → FINISHED（`calculate():1448-1458`）。
#[test]
fn run_channels_switches_in_order() {
    let mut g = FakeGrid::new(2, 2);
    g.put(1, 1, 19, true);
    assert!(run_channels(&mut g).is_ok());
    assert_eq!(
        g.engine,
        vec![state::FIRST, state::SECOND, state::SPECIAL, state::FINISHED]
    );
}
