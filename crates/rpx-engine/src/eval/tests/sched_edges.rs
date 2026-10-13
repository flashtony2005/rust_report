//! 通道边界型用例（自 `tests.rs` 拆出，门禁行数）。
//!
//! 这一组不是"再跑一遍 happy path"，而是钉住**通道实现里两处最容易写错的结构性约束**：
//! 循环边界每轮重读、以及终局错误必须穿透。两条都是 2026-10-13 顶层接线时发现的，
//! 出处见各用例的文档注释。

use std::collections::VecDeque;

use super::super::sched::{channel4, channel5, run_channels};
use super::harness::FakeGrid;
use crate::error::EngineError;

/// ⛔ **扩展中途插入的行列必须被本趟扫到**：pass4/5 的循环边界是**每轮重读**的
/// `_$16`/`_$15`（`ExtCellSet.java:1304`/`:1369`/`:1371`），不是循环外的快照。
///
/// 把边界缓存到循环外（Rust 的 `for i in 1..rows` 天然如此）会**漏扫新插入的行列**，
/// 后果是「扩展出来的整段空白」。本用例就是那条约束的钉子 ——
/// 假网格初始只有 (1,1)，算它时把两个尺寸各 +1，于是 (2,1)/(1,2) 才进入视野。
#[test]
fn channels_rescan_rows_and_cols_inserted_mid_pass() {
    // 通道 4（18）
    let mut g = FakeGrid::new(2, 2);
    let a = g.put(1, 1, 18, true);
    g.cells[a.0 as usize].grow = true;
    let b = g.cell(2, 1, 18, true); // 插入后才出现的新行
    g.at.insert((2, 1), b);
    let c = g.cell(1, 2, 18, true); // 插入后才出现的新列
    g.at.insert((1, 2), c);
    assert!(channel4(&mut g, &mut Vec::new()).is_ok());
    assert!(g.prep.contains(&b), "通道 4：新插入的行必须被重扫到");
    assert!(g.prep.contains(&c), "通道 4：新插入的列必须被重扫到");

    // 通道 5（17）同一条约束
    let mut g = FakeGrid::new(2, 2);
    let a = g.put(1, 1, 17, true);
    g.cells[a.0 as usize].grow = true;
    let b = g.cell(2, 1, 17, true);
    g.at.insert((2, 1), b);
    let c = g.cell(1, 2, 17, true);
    g.at.insert((1, 2), c);
    assert!(channel5(&mut g, &mut VecDeque::new(), &mut Vec::new()).is_ok());
    assert!(g.prep.contains(&b), "通道 5：新插入的行必须被重扫到");
    assert!(g.prep.contains(&c), "通道 5：新插入的列必须被重扫到");
}

/// ⛔ **终局错误不得被压成"完成"**：`calculate` 抛出的真错误必须穿透每条通道。
///
/// 这是 `calculate` 签名从 `Result<(), Suspend>` 改成 `Result<(), Flow>` 的**唯一理由**。
/// 若某条通道写成 `Err(e) => { … }` 而把 Fatal 也当成挂起消化掉，本用例立刻红。
#[test]
fn a_fatal_error_from_calculate_pierces_every_channel() {
    // 通道 4（19 优先）
    let mut g = FakeGrid::new(2, 2);
    let a = g.put(1, 1, 19, true);
    g.cells[a.0 as usize].fatal = true;
    let want = format!("fatal#{}", a.0);
    assert!(
        matches!(run_channels(&mut g), Err(EngineError::UnknownCell { id }) if id == want),
        "通道 4 必须原样抛出 Fatal"
    );

    // 通道 5（17）
    let mut g = FakeGrid::new(2, 2);
    let b = g.put(1, 1, 17, true);
    g.cells[b.0 as usize].fatal = true;
    let want = format!("fatal#{}", b.0);
    assert!(
        matches!(run_channels(&mut g), Err(EngineError::UnknownCell { id }) if id == want),
        "通道 5 必须原样抛出 Fatal"
    );
}
