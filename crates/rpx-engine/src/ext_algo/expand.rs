//! `expandLeftHead` / `expandTopHead`（附件 I.8；`ExtNormalCell.java:462-490`、
//! `ExtExtendCell.java:214-240`）—— M2-04「模板合并区跨进扩展行」块高的来源。
//!
//! # 结构（两方向完全对称）
//!
//! 1. **1 参版**从锚点的 head **字段**起步沿链走（起点 = `this.leftHead`，
//!    **不含锚点自身**）；
//! 2. 链上每个节点调 **3 参钩子**：扩展格节点遍历**自己的子格列表**
//!    （left → `_$6`，top → `_$5`），对 `sub1 != sub && contain*(cur)` 的成员
//!    `add*Merge(inc)` 并重铺矩形；**非扩展格节点的钩子是空实现**
//!    （`ExtCell.java:209-214`）⇒ 不扇出；
//! 3. 回到 1 参版：节点自身若 `contain*(ExtCell)`（**整块包含**语义，
//!    不同于 `containRow(int)` 的行区间相交！）也 `add*Merge(inc)` 并重铺。
//!    `containCol(ExtCell)` 带 `sameRowTypeAs` 前置（`ExtNormalCell.java:283-287`）
//!    —— 这是 `engine_ext` 里 cell00 不被撑大的守卫之一（设计文档 §6.3 守卫 ②）。
//!
//! 代入 `engine_ext`（cols=1, count=5 ⇒ inc=4）：锚点 topHead 链 = [cell00] ⇒
//! 只有 `cell00` 的 3 参钩子被调 ⇒ 扫 `cell00._$5` ⇒ 命中 (1,1)(2,1)(3,1)
//! 各 `addColMerge(4)` → cm=1+4=5；锚点自身被 `sub1 == sub` 挡掉（守卫 ①）；
//! cell00 被列包含失败挡掉。探针实证见设计文档 §6.3。

use crate::arena::CellId;
use crate::heads::{HeadGrid, HeadState};

/// `containRow(ExtCell)`（`ExtNormalCell.java:278-281`）：sub 的**整块**落在
/// head 块内（与 `contain_row(int)` 的"行号落区间"是两个重载，勿混）。
pub(crate) fn contain_row_band(g: &HeadState, head: CellId, sub: CellId) -> bool {
    let h = &g.cells[head.0 as usize];
    let s = &g.cells[sub.0 as usize];
    h.row <= s.row && s.row + s.row_merge <= h.row + h.row_merge
}

/// `containCol(ExtCell)`（`:283-287`）：**带 `sameRowTypeAs` 前置** + 列整块包含。
pub(crate) fn contain_col_band(g: &HeadState, head: CellId, sub: CellId) -> bool {
    if !g.same_row_type_as(head, sub) {
        return false;
    }
    let h = &g.cells[head.0 as usize];
    let s = &g.cells[sub.0 as usize];
    h.col <= s.col && s.col + s.col_merge <= h.col + h.col_merge
}

/// `expandLeftHead(mergeInc)`（`ExtNormalCell.java:462-476`）：沿锚点左 head 链撑大。
pub(crate) fn expand_left_head(g: &mut HeadState, anchor: CellId, inc: i32) {
    let mut sub = anchor;
    let mut head = g.cells[anchor.0 as usize].left_head;
    while head.is_some() {
        // ① 3 参钩子：扩展格节点向自己的左子格扇出（`ExtExtendCell.java:214-226`）
        if g.cells[head.0 as usize].is_extender {
            let subs = g.cells[head.0 as usize].left_subs.clone();
            for s1 in subs {
                if s1 == sub || !contain_row_band(g, s1, anchor) {
                    continue;
                }
                g.add_row_merge(s1, inc);
                refill(g, s1);
            }
        }
        // ② 节点自身整块包含锚点行带 → 撑大并重铺（`:467-472`）
        if contain_row_band(g, head, anchor) {
            g.add_row_merge(head, inc);
            refill(g, head);
        }
        let next = g.cells[head.0 as usize].left_head;
        sub = head;
        head = next;
    }
}

/// `expandTopHead(mergeInc)`（`:477-490`）：沿锚点上 head 链撑大。与上对称，
/// 钩子遍历 `top_subs`、判定用 `containCol`（带 sameRowTypeAs）。
pub(crate) fn expand_top_head(g: &mut HeadState, anchor: CellId, inc: i32) {
    let mut sub = anchor;
    let mut head = g.cells[anchor.0 as usize].top_head;
    while head.is_some() {
        if g.cells[head.0 as usize].is_extender {
            let subs = g.cells[head.0 as usize].top_subs.clone();
            for s1 in subs {
                if s1 == sub || !contain_col_band(g, s1, anchor) {
                    continue;
                }
                g.add_col_merge(s1, inc);
                refill(g, s1);
            }
        }
        if contain_col_band(g, head, anchor) {
            g.add_col_merge(head, inc);
            refill(g, head);
        }
        let next = g.cells[head.0 as usize].top_head;
        sub = head;
        head = next;
    }
}

/// `_$1(startRow, startCol, startRow+rowMerge, startCol+colMerge, cell)`
/// （`ExtCellSet.java:706-712`）：把格的合并块在新跨度上重铺一遍（半开区间）。
fn refill(g: &mut HeadState, id: CellId) {
    let (r, c, rm, cm) = {
        let x = &g.cells[id.0 as usize];
        (x.row, x.col, x.row_merge, x.col_merge)
    };
    g.fill_rect(r, c, r + rm, c + cm, id);
}
