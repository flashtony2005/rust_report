//! `ds_current` 的单测：全部对照 `docs/m2-engine-design.md` §5.2 的实测锚点。
//!
//! 假宿主在 [`harness`]（`_$4` 与 `_$7` 是两个独立字段，见其模块文档）。

mod harness;

use self::harness::{Fake, FakeCell};
use super::types::{DsInstance, HeadPairTracker, NULL_ROW};
use super::{
    reset_ds_current, row_cross_default, set_ds_current, set_ds_current_for_cell,
    set_ds_current_top_head,
};
use crate::arena::CellId;

// ───────────────────────── `_$19` 跟踪器 ─────────────────────────

#[test]
fn tracker_starts_changed_and_stays_changed_while_it_is_changed() {
    let mut t = HeadPairTracker::new();
    assert!(t.is_changed(), "构造时 _$1 = true（`lIlIlllIIllllIII.java:11`）");
    t.set_cell(CellId(7), CellId(8), CellId::ROOT);
    assert!(t.is_changed(), "changed 为真时只更新记录、保持真，不比较");
}

#[test]
fn tracker_marks_a_new_head_region_and_goes_quiet_inside_one() {
    let c00 = CellId::ROOT;
    let (a, b) = (CellId(10), CellId(11));
    let mut t = HeadPairTracker::new();
    t.set_changed(false);

    t.set_cell(c00, c00, c00);
    assert!(t.is_changed(), "两者都是 cell00 视作变化");

    t.set_changed(false);
    t.set_cell(a, c00, c00);
    assert!(t.is_changed(), "left 由 cell00 变成 a ⇒ 换区");

    t.set_changed(false);
    t.set_cell(a, c00, c00);
    assert!(!t.is_changed(), "同一 (left, top) 对 ⇒ 同区，无需重算 DS");

    t.set_changed(false);
    t.set_cell(b, c00, c00);
    assert!(t.is_changed(), "left 换成 b ⇒ 换区");

    t.set_changed(false);
    t.set_cell(b, CellId(12), c00);
    assert!(t.is_changed(), "left 不变但 top 变 ⇒ 换区");

    t.set_changed(false);
    t.set_cell(b, CellId(12), c00);
    assert!(!t.is_changed());
}

#[test]
fn tracker_reset_clears_both_pointers_and_the_flag() {
    let mut t = HeadPairTracker::new();
    t.set_cell(CellId(3), CellId(4), CellId::ROOT);
    t.reset();
    assert!(!t.is_changed(), "reset(): _$1 = false（`:49-53`）");
    // reset 后记录被清空 ⇒ 任何一个非 cell00 的 left 都会再次判"变化"
    t.set_cell(CellId(3), CellId(4), CellId::ROOT);
    assert!(t.is_changed());
}

// ───────────────────── `resetDSCurrent` / `setDSCurrent` ─────────────────────

#[test]
fn reset_clears_every_dataset_and_marks_left_top() {
    let mut f = Fake::new(2);
    f.ds[0].set(Some(DsInstance::new(0, 1)), false);
    f.ds[1].set(Some(f.root[1]), false);
    reset_ds_current(&mut f);
    for ds in 0..2 {
        assert_eq!(f.cur(ds), None, "resetDSCurrent 清的是 ctx 里**全部** DS（`:556-563`）");
        assert!(f.ds[ds].is_left_top, "第二条实参是 true（`:561`）");
        assert_eq!(f.ds[ds].current_row, 1, "setCurrent(g) 把 _$5 复位成 1（`:365`）");
    }
    assert!(!f.tracker.is_changed(), "第一句是 _$19.setChanged(false)（`:555`）");
}

#[test]
fn fill_only_null_makes_unreferenced_datasets_fall_back_to_the_root_group() {
    // engine_sum：ds1 被 head 链灌成 Row(0)；ds2 没人管 ⇒ 退化成全量聚合（450）
    let mut f = Fake::new(2);
    f.ds[0].set(Some(DsInstance::new(0, 0)), true);
    set_ds_current(&mut f);
    assert_eq!(f.cur(0), Some(DsInstance::new(0, 0)), "已经是 Row 的不动（'只填空'）");
    assert_eq!(f.cur(1), Some(f.root[1]), "current == null ⇒ 填 getRootGroup()");
}

// ───────────────────── head 链行走（本模块的核心） ─────────────────────

/// `engine_sum` 行 2 的骨架：`A1` 是 18 锚点，`follow` 是同带的"跟随型"扩展格
/// （`ds1.select(qty)`：扩展改写后 `extend=18`，但求值成标量 `Row` ⇒ 不扩展、`_$7 == null`），
/// `tail` 是链尾的 `ds2.sum(qty)`。
///
/// 返回 `(fake, follow, tail, a1)`；id 顺序 = `0:cell00 / 1:a1 / 2:a2 / 3:a3 / 4:follow / 5:tail`。
fn engine_sum_shape() -> (Fake, CellId, CellId, CellId) {
    let mut f = Fake::new(2);
    let a1 = f.add(FakeCell::extender(Some(DsInstance::new(0, 0)), Vec::new()));
    let a2 = f.add(FakeCell::extender(Some(DsInstance::new(0, 1)), Vec::new()));
    let a3 = f.add(FakeCell::extender(Some(DsInstance::new(0, 2)), Vec::new()));
    let shared = vec![a1, a2, a3]; // 锚点与克隆共享同一个 `_$7`
    for id in &shared {
        f.cells[id.0 as usize].ext = shared.clone();
    }
    let follow = f.add(FakeCell::extender(Some(DsInstance::new(0, 0)), Vec::new()));
    f.cells[follow.0 as usize].left = a1;
    let tail = f.add(FakeCell::plain());
    f.cells[tail.0 as usize].left = follow;
    (f, follow, tail, a1)
}

#[test]
fn a_follower_extender_without_ext_cells_still_installs_its_row() {
    let (mut f, follow, tail, _) = engine_sum_shape();

    // ① 跟随格自己求值前：self 段用**它自己的** `_$4` 灌 ds1
    set_ds_current_for_cell(&mut f, follow).unwrap();
    assert_eq!(f.cur(0), Some(DsInstance::new(0, 0)), "跟随格的 self 段（`:295-297`）");
    assert_eq!(f.cur(1), Some(f.root[1]), "ds2 不在链上 ⇒ 全量");

    // ② 链尾格：靠 left 链走到跟随格，同样拿到 Row(0)
    set_ds_current_for_cell(&mut f, tail).unwrap();
    assert_eq!(f.cur(0), Some(DsInstance::new(0, 0)), "left 链上的扩展格会灌 Row");
    assert_eq!(f.cur(1), Some(f.root[1]));
}

#[test]
fn no_extender_on_the_chain_means_the_dataset_stays_at_the_root() {
    // 把跟随格降级成"非扩展格"并**截断**链（left = NONE）⇒ 链上没有任何格能灌 Row。
    // ⚠ 只降级不截断是**测不出**的：链会继续走到锚点克隆，那里仍会灌同一个 Row。
    let (mut f, follow, tail, _) = engine_sum_shape();
    f.cells[follow.0 as usize].extender = false;
    f.cells[follow.0 as usize].left = CellId::NONE;
    set_ds_current_for_cell(&mut f, tail).unwrap();
    assert_eq!(f.cur(0), Some(f.root[0]), "非扩展格不覆写 setDSCurrentLeftHead（`ExtCell.java:215-217`）");
    assert_eq!(f.cur(1), Some(f.root[1]));
}

#[test]
fn the_row_installed_is_the_one_of_the_reached_clone_not_the_last_one() {
    // engine_sum 行 3：链上的扩展格是 A2（`_$4 = Row(1)`）⇒ ds1 得到 200 而不是 150
    let (mut f, follow, tail, _) = engine_sum_shape();
    let a2 = f.id(2);
    f.cells[follow.0 as usize].left = a2;
    f.cells[follow.0 as usize].inst = Some(DsInstance::new(0, 1));
    set_ds_current_for_cell(&mut f, tail).unwrap();
    assert_eq!(f.cur(0), Some(DsInstance::new(0, 1)), "链上先撞上 A2（`_$4 = Row(1)`）");
    set_ds_current_for_cell(&mut f, follow).unwrap();
    assert_eq!(f.cur(0), Some(DsInstance::new(0, 1)), "跟随格自己的 `_$4` 也是 Row(1)");
}

#[test]
fn the_self_segment_has_no_null_guard_so_the_last_writer_wins() {
    // `ExtExtendCell.setDSCurrent:295-304` 的 self 段两节都**没有** `current == null`
    // 守卫（与 `setDSCurrentLeftHead:251/:257` 不同）⇒ 后写的 `_$7` 成员覆盖先写的自身。
    let mut f = Fake::new(1);
    let other = f.add(FakeCell::extender(Some(DsInstance::new(0, 1)), Vec::new()));
    let me = f.add(FakeCell::extender(Some(DsInstance::new(0, 3)), vec![other]));
    set_ds_current_for_cell(&mut f, me).unwrap();
    assert_eq!(f.cur(0), Some(DsInstance::new(0, 1)), "自身先写、成员后写 ⇒ 成员赢");
}

// ───────────────────── `setDSCurrentTopHead` 的两条分支 ─────────────────────

#[test]
fn the_top_head_guard_blocks_a_second_writer() {
    let mut f = Fake::new(1);
    let t1 = f.add(FakeCell::extender(Some(DsInstance::new(0, 2)), Vec::new()));
    let t2 = f.add(FakeCell::extender(Some(DsInstance::new(0, 1)), Vec::new()));
    reset_ds_current(&mut f); // 全部 is_left_top = true
    assert!(f.ds[0].is_left_top, "resetDSCurrent 把 _$4 置真（`:561`）");

    set_ds_current_top_head(&mut f, t1).unwrap();
    assert_eq!(f.cur(0), Some(DsInstance::new(0, 2)));
    assert!(!f.ds[0].is_left_top, "top 分支写的是 false（`:272`/`:286`）");

    set_ds_current_top_head(&mut f, t2).unwrap();
    assert_eq!(f.cur(0), Some(DsInstance::new(0, 2)), "守卫 isLeftTop() 已假 ⇒ 第二个 top 头不生效");
}

#[test]
fn the_top_head_with_an_existing_current_goes_through_cross() {
    let mut f = Fake::new(1);
    let t1 = f.add(FakeCell::extender(Some(DsInstance::new(0, 2)), Vec::new()));
    f.ds[0].set(Some(DsInstance::new(0, 5)), true); // 已有一个 Row，且 is_left_top 真
    set_ds_current_top_head(&mut f, t1).unwrap();
    assert_eq!(f.cur(0), Some(DsInstance::new(0, NULL_ROW)), "不同行 ⇒ cross 退化成 getNullRow()");
}

#[test]
fn row_cross_returns_self_for_the_same_row_and_null_row_otherwise() {
    let f = Fake::new(1);
    let same = DsInstance::new(0, 3);
    assert_eq!(row_cross_default(&f, 0, same, same).unwrap(), same);
    assert_eq!(row_cross_default(&f, 0, same, DsInstance::new(0, 4)).unwrap(), f.null[0]);
}
