//! `extendCol(cell, count)`（`ExtCellSet.java:1083-1165`）—— 横向扩展（19）。
//!
//! 与 `extendRow` 的不对称点（附件 I.6/I.7，全部有源码行号）：
//! 1. **前置校验**：锚点所在行必须是"行头行"，否则报
//!    `ExtCellSet.entendCol`（产品拼写错误，逐字保留 → `EngineError::EntendCol`）；
//! 2. **第 0 行列头是显式逐格克隆的**（`:1098-1104`）；
//! 3. 第 1 趟对 `leftHead == root` 的克隆 **直接 add 进 root 的左子格列表**
//!    （`:1133-1135`）—— 不走 setCellLeftHead（不改指针，只登记）；
//! 4. 第 2 趟 **topHead 平移、leftHead 三分支**（root 不动 / 带内平移 /
//!    带外原样）—— 与 extendRow 正好相反。
//!
//! ⚠ 第 2 趟 `top.getRow()` 在 top 为空时 Java 直接 NPE —— Rust 侧报
//! `EngineError::Invariant`（head 图保证链上有 root 兜底，此为不可能状态）。

use crate::arena::CellId;
use crate::error::{EngineError, Result};
use crate::ext_algo::{envelope, expand, insert, is_top_head_of};
use crate::heads::HeadState;

/// 横向扩展：把锚点的列带复制 `count - 1` 份向右堆叠。
pub fn extend_col(g: &mut HeadState, anchor: CellId, count: i32) -> Result<()> {
    if count < 1 {
        return Ok(()); // 附件 I.1c/I.10
    }
    // 前置校验（`:1086-1090`）：只有行头行上的格允许列扩展
    let arow = g.cells[anchor.0 as usize].row;
    if !envelope::is_row_header(g, arow) {
        return Err(EngineError::EntendCol { row: arow });
    }
    let begin_col = envelope::begin_col(g, anchor);
    let end_col = envelope::end_col(g, anchor);
    let cols = end_col - begin_col;

    // Java 的 `_$3` 是数组引用共享：锚点与全部克隆指向同一对象。Rust 侧因此
    // 只在循环后建一次 `Arc`；逐克隆 `to_vec()` = n²×4 字节（50k ≈ 9.9 GB）。
    let mut ext_cells = vec![CellId::NONE; count as usize];
    ext_cells[0] = anchor;

    // ① 插列（部分平移，带 = 首个..最后一个行头行；alignColHead = true，`:1097`）
    let (first, last) = (envelope::first_header_row(g), envelope::last_header_row(g));
    insert::insert_cols(g, end_col, cols * (count - 1), first, last, true);

    // ② 显式克隆第 0 行的每个带列（`:1098-1104`）
    for j2 in begin_col..end_col {
        let sub2 = g.at_pos(0, j2);
        if sub2.is_some() {
            for m in 1..count {
                let cl = g.clone_cell(sub2);
                g.add_col(cl, m * cols);
                g.set_pos(0, j2 + m * cols, cl);
            }
        }
    }
    // ③ expandTopHead —— 撑大的是"上 head 链节点 + 各自 top_subs 里
    //    containCol(锚点) 的成员"（附件 I.8；设计文档 §6.3 的探针实证）
    expand::expand_top_head(g, anchor, cols * (count - 1));
    let irow = g.row_size; // 循环外取一次：插列不改行数（`:1106`）

    // ── 第 1 趟：克隆（`:1107-1140`）──
    for i in 1..irow {
        let mut j = begin_col;
        while j < end_col {
            let sub = g.at_pos(i, j);
            if sub.is_none() {
                j += 1;
                continue;
            }
            let owned = {
                let s = &g.cells[sub.0 as usize];
                s.row == i && s.col == j && is_top_head_of(g, anchor, sub)
            };
            if owned {
                for m in 1..count {
                    let new_sub = g.clone_cell(sub);
                    g.add_col(new_sub, m * cols);
                    let (rm, cm) = (
                        g.cells[new_sub.0 as usize].row_merge,
                        g.cells[new_sub.0 as usize].col_merge,
                    );
                    let new_col = j + m * cols;
                    // 铺满 [i, i+rm) × [newCol, newCol+cm)（`:1120-1125`）
                    g.fill_rect(i, new_col, i + rm, new_col + cm, new_sub);
                    if sub == anchor {
                        ext_cells[m as usize] = new_sub;
                    }
                    // leftHead == root 的克隆登记进 root 的左子格列表
                    //（`:1133-1135`；⛔ 直接 add，不改指针）
                    if g.cells[sub.0 as usize].left_head == CellId::ROOT {
                        g.cells[CellId::ROOT.0 as usize].left_subs.push(new_sub);
                    }
                }
            }
            j += g.cells[sub.0 as usize].col_merge; // 跳块（`:1136`）
        }
    }

    // ── 第 1 趟收尾：数组已填满，建一次共享分配并分发（`:1126-1140`）──
    // 锚点与每个克隆共用同一条 `Arc<[CellId]>`：内存 O(n)、克隆 O(1)。
    let shared: std::sync::Arc<[CellId]> = ext_cells.into();
    g.set_ext_cells(anchor, std::sync::Arc::clone(&shared));
    for m in 1..count {
        let cl = shared[m as usize];
        if cl.is_some() {
            g.set_ext_cells(cl, std::sync::Arc::clone(&shared));
        }
    }

    // ── 第 2 趟：修 head（`:1141-1164`）──
    for i in 1..irow {
        let mut j = begin_col;
        while j < end_col {
            let sub = g.at_pos(i, j);
            if sub.is_none() {
                j += 1;
                continue;
            }
            let owned = {
                let s = &g.cells[sub.0 as usize];
                s.row == i && s.col == j && is_top_head_of(g, anchor, sub)
            };
            if owned {
                let left = g.cells[sub.0 as usize].left_head;
                let top = g.cells[sub.0 as usize].top_head;
                if top.is_none() {
                    // Java 在 `top.getRow()` 处 NPE（`:1149`）；不可能状态显式报错
                    return Err(EngineError::Invariant {
                        detail: format!("cell#{} in extendCol pass2 has null topHead", sub.0),
                    });
                }
                let (xrow, xcol) = {
                    let t = &g.cells[top.0 as usize];
                    (t.row, t.col)
                };
                for m in 1..count {
                    let new_sub = g.at_pos(i, j + cols * m);
                    if sub != anchor {
                        // topHead：**平移**到 top 同行 +cols*m（`:1151-1155`）
                        let new_head = g.at_pos(xrow, xcol + cols * m);
                        g.set_cell_top_head(new_sub, new_head)?;
                    }
                    if left == CellId::ROOT {
                        continue; // root 不动（`:1156-1157`）
                    }
                    let lcol = g.cells[left.0 as usize].col;
                    if lcol >= begin_col && lcol < end_col {
                        // 带内：平移（`:1158-1160`）
                        let lrow = g.cells[left.0 as usize].row;
                        let new_head = g.at_pos(lrow, lcol + cols * m);
                        g.set_cell_left_head(new_sub, new_head)?;
                    } else {
                        // 带外：原样保留（`:1161-1162`）
                        g.set_cell_left_head(new_sub, left)?;
                    }
                }
            }
            j += g.cells[sub.0 as usize].col_merge;
        }
    }
    Ok(())
}
