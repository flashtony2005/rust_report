//! head 查找的移植逻辑单测（手工构造网格，不依赖夹具）。
//!
//! # 为什么现在只有单测、夹具回归还没接
//!
//! 夹具的 `expected.json` 里，`source` 块写的是**引擎矩阵在该位置的格**
//! （`CalcJson.java:92` 的 `ecs.getSource(r,c)`），而 `left_head`/`top_head`
//! 真值写的是**计算格自身的 `getSourceRow()`**（克隆实例指向其引擎原点）——
//! 两套坐标系并存。要驱动夹具级回归，必须先由阶段 B（扩展算法）建出真实
//! 计算网格，届时 head 真值与实现输出天然同坐标系。在此之前硬建模等于猜，
//! 本文件先把**移植逻辑本身**钉死：
//!
//! 1. 命中即返回（`engine_sum` 的 8 个链式反例的语义）——测试 1；
//! 2. `_$2` 的行类型硬门槛（`-96` 正反对照，`engine_head_top` 的判别式）——测试 3；
//! 3. head-of-head 分支及其副作用（`:697` 递归回填）——测试 4；
//! 4. 分支 A 走**引擎矩阵**解析、且自指报环——测试 5/6；
//! 5. 同族集合与跨度判定——测试 7/8。
//!
//! ⛔ `_$3` 的 `:653` 守卫（`is_merged && ext != 18 && row != xrow`）在本测试
//! 框架里**不可构造**：扫描用 `getCell(xrow, col)` 取格，取出的格行号恒等于
//! `xrow`。该守卫是否为产品死代码，待阶段 B 落地后核对，**不要**为凑覆盖
//! 而放宽 `HeadGrid` 的抽象去硬造。

use super::cell::HeadCell;
use super::find::{find_left_head, find_top_head, HeadGrid};
use super::grid::HeadState;
use crate::arena::CellId;

/// 空表：第 0 个格是 `cell00`（`is_extender` 必须为真，见 `find.rs` trait 文档）。
fn grid() -> HeadState {
    let mut g = HeadState::new();
    let mut root = HeadCell::new(0, 0, 16, -91);
    root.is_extender = true;
    g.push(root);
    g
}

/// 快捷推入一格（跨度 1、无声明、无 head）。
fn add(g: &mut HeadState, row: i32, col: i32, ext: i8, rt: i8) -> CellId {
    g.push(HeadCell::new(row, col, ext, rt))
}

#[test]
fn left_head_returns_the_nearest_extender_hit_first_not_the_farthest() {
    // `engine_sum` 的链式语义：`(2,3)..(2,9)` 的 head 是 (2,2) 而非 (2,1)。
    // `_$3` 从 col-1 递减扫，**命中即返回**；(2,2) 自己是 18 扩展格 ⇒ 先命中它。
    let mut g = grid();
    let a = add(&mut g, 2, 1, 18, -91);
    let b = add(&mut g, 2, 2, 18, -91);
    let c = add(&mut g, 2, 3, 16, -91);
    g.set_current(c);
    assert_eq!(
        find_left_head(&mut g, c).unwrap(),
        b,
        "必须命中最近的 (2,2)"
    );
    g.set_current(b);
    assert_eq!(find_left_head(&mut g, b).unwrap(), a);
}

#[test]
fn both_axes_fall_back_to_cell00_when_no_candidate_qualifies() {
    // 扫描区内没有任何合格候选 ⇒ 兜底 `cell00`（`:665` / `:703`）。
    let mut g = grid();
    let c = add(&mut g, 2, 2, 16, -91);
    g.set_current(c);
    assert_eq!(find_left_head(&mut g, c).unwrap(), g.cell00());
    assert_eq!(find_top_head(&mut g, c).unwrap(), g.cell00());
}

#[test]
fn top_head_row_type_gate_skips_a_19_extender_of_a_different_family() {
    // `engine_head_top` 的 `-96` 判别式：(2,1) 是 19 扩展格但行类型 -96
    // ⇒ `sameRowTypeAs` 假 ⇒ `:695` 硬门槛跳过它 ⇒ 落到 (1,1)。
    let mut g = grid();
    let r1 = add(&mut g, 1, 1, 19, -91);
    let _r2 = add(&mut g, 2, 1, 19, -96);
    let r3 = add(&mut g, 3, 1, 16, -91);
    g.set_current(r3);
    assert_eq!(
        find_top_head(&mut g, r3).unwrap(),
        r1,
        "-96 必须被行类型门槛跳过"
    );

    // 对照组：同一结构、(2,1) 改回同族 -91 ⇒ 命中 (2,1) 本身。
    // 两条一起钉，防「把门槛删掉/放宽」与「把命中分支写死」两个方向的错误。
    let mut g2 = grid();
    let _a1 = add(&mut g2, 1, 1, 19, -91);
    let a2 = add(&mut g2, 2, 1, 19, -91);
    let a3 = add(&mut g2, 3, 1, 16, -91);
    g2.set_current(a3);
    assert_eq!(find_top_head(&mut g2, a3).unwrap(), a2);
}

#[test]
fn top_head_head_of_head_branch_returns_the_chain_head_and_fills_it() {
    // S1 形态：(2,1) 不是 19 扩展格，但它已有上 head ⇒ 走 head-of-head 分支
    // 返回链头 (1,1)，且 `:697` 的递归把 (1,1) 的上 head 兜底成 cell00（副作用）。
    let mut g = grid();
    let t1 = add(&mut g, 1, 1, 19, -91);
    let t2 = add(&mut g, 2, 1, 16, -91);
    g.set_top_head(t2, t1);
    let t3 = add(&mut g, 3, 1, 16, -91);
    g.set_current(t3);
    assert_eq!(find_top_head(&mut g, t3).unwrap(), t1);
    assert_eq!(g.top_head(t1), g.cell00(), ":697 递归必须回填链头的上 head");
}

#[test]
fn declared_left_head_resolves_via_the_engine_matrix_not_the_scan() {
    // 分支 A：声明的 "A1" 经**引擎矩阵**（by_src）解析，与计算态扫描无关。
    // 目标格故意放在 (9,9)——扫描永远到不了；若实现误走扫描，只会返回 cell00。
    let mut g = grid();
    let target = add(&mut g, 9, 9, 16, -91);
    g.index_src((1, 1), target);
    let mut hc = HeadCell::new(2, 3, 16, -91);
    hc.declared_left = Some("A1".into());
    let c = g.push(hc);
    g.set_current(c);
    assert_eq!(find_left_head(&mut g, c).unwrap(), target);
}

#[test]
fn declared_left_head_pointing_at_itself_is_a_circular_error() {
    // `:638-646`：声明的 head 解析后 `isLeftHeadOf(自身)` 恒真（链上溯自含）
    // ⇒ 必须报环，而不是返回自己。
    let mut g = grid();
    let mut hc = HeadCell::new(2, 2, 16, -91);
    hc.declared_left = Some("B2".into());
    let c = g.push(hc);
    g.index_src((2, 2), c);
    g.set_current(c);
    assert!(find_left_head(&mut g, c).is_err(), "自指声明必须报环");
}

#[test]
fn same_row_type_family_matches_the_java_set() {
    // `sameRowTypeAs:266-269`：{-90,-91,-94,-95} 同族互真；-96 只与严格相等配。
    let mut g = grid();
    let a = add(&mut g, 1, 1, 16, -91);
    let b = add(&mut g, 1, 2, 16, -95);
    let c = add(&mut g, 1, 3, 16, -96);
    let d = add(&mut g, 1, 4, 16, -90);
    let e = add(&mut g, 1, 5, 16, -94);
    assert!(g.same_row_type_as(a, b), "-91 与 -95 同族");
    assert!(!g.same_row_type_as(a, c), "-91 与 -96 必须判异");
    assert!(g.same_row_type_as(c, c), "-96 与自身严格相等");
    assert!(g.same_row_type_as(d, e), "-90 与 -94 同族");
}

#[test]
fn contain_row_and_contain_col_follow_the_cell_span() {
    // `containRow:250-252` / `containCol:273-275`：从自身坐标起、跨自身 Merge 宽度。
    let mut g = grid();
    let mut hc = HeadCell::new(2, 1, 16, -91);
    hc.row_merge = 2;
    hc.col_merge = 2;
    let m = g.push(hc);
    assert!(g.is_merged(m));
    assert!(g.contain_row(m, 2) && g.contain_row(m, 3));
    assert!(!g.contain_row(m, 4));
    assert!(g.contain_col(m, 1) && g.contain_col(m, 2));
    assert!(!g.contain_col(m, 3));
}

#[test]
fn address_parse_accepts_dollar_forms_and_rejects_garbage() {
    // `getSource("B2")` 的解析替身：`$` 直接剥掉；非法串返回 NONE 而非 panic。
    let mut g = grid();
    let target = add(&mut g, 9, 9, 16, -91);
    g.index_src((2, 2), target);
    assert_eq!(g.get_source_by_id("B2"), target);
    assert_eq!(g.get_source_by_id("$B$2"), target, "绝对引用形式等价");
    assert!(g.get_source_by_id("no_such_thing").is_none());
    assert!(g.get_source_by_id("").is_none());
}
