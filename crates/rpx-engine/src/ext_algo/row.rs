//! `extendRow(cell, count)`（`ExtCellSet.java:981-1057`）—— 纵向扩展（18）。
//!
//! 逐行对照表见附件 I.5；易错点三条：
//! 1. `isLeftHeadOf` **自含**（从 sub 自身起步）⇒ 锚点天然被克隆；
//! 2. 只克隆"左 head 链覆盖到"的格，合并区靠铺满**整块**补回；
//! 3. 第 2 趟 **leftHead 平移、topHead 原样共享**（root 除外）——
//!    与 `extendCol` 的"带内/带外三分支"是最容易写错的不对称点（附件 I.7）。
//!
//! ⚠ `_$8()` 中断检查（每轮循环一次，附件 I.4）待顶层 `calculate()` 接线时补挂。

use crate::arena::CellId;
use crate::error::Result;
use crate::ext_algo::{envelope, expand, insert, is_left_head_of};
use crate::heads::HeadState;

/// 纵向扩展：把锚点的行带复制 `count - 1` 份向下堆叠。
///
/// `count` = 锚点表达式结果 List 的长度（附件 I.1a），**不是**"数据集行数"。
pub fn extend_row(g: &mut HeadState, anchor: CellId, count: i32) -> Result<()> {
    if count < 1 {
        return Ok(()); // 附件 I.1c/I.10：上游保证 ≥1；Rust 侧不继承越界假设
    }
    let begin_row = envelope::begin_row(g, anchor);
    let end_row = envelope::end_row(g, anchor);
    let rows = end_row - begin_row;

    // extCells = new ExtCell[count]; extCells[0] = cell（`:994-996`）
    // ⛔ Java 里这条数组是**引用共享**的：锚点与每个克隆的 `_$3` 指向同一对象，
    // 循环中填入后续实例时所有持有者都看得到。Rust 侧因此只在循环后建一次
    // `Arc` 再分发；逐克隆 `to_vec()` 会造成 n²×4 字节的 O(n²) 内存
    // （50k 明细实测 ≈ 9.9 GB，见 `scripts/bench_detail_mem.py`）。
    let mut ext_cells = vec![CellId::NONE; count as usize];
    ext_cells[0] = anchor;

    // ① 在 endRow（行带之后）插 rows*(count-1) 行 —— 块高被乘进去（`:997`）
    insert::insert_rows(g, end_row, rows * (count - 1));
    // ② 沿左 head 链撑大 rowMerge（M2-04 的机制，附件 I.8）
    expand::expand_left_head(g, anchor, rows * (count - 1));
    let icol = g.col_size;

    // ── 第 1 趟：克隆（`:1000-1029`）──
    for m in 1..count {
        let k = m * rows;
        for i in begin_row..end_row {
            let x = i + k;
            // cells2[0] = clone(cells1[0]); addRow(k) —— 第 0 列脚手架，每带行一格
            let scaffold = g.at_pos(i, 0);
            if scaffold.is_some() {
                let cl = g.clone_cell(scaffold);
                g.add_row(cl, k);
                g.set_pos(x, 0, cl);
            }
            let mut j = 1;
            while j < icol {
                let sub = g.at_pos(i, j);
                if sub.is_none() {
                    j += 1;
                    continue;
                }
                let owned = {
                    let s = &g.cells[sub.0 as usize];
                    s.row == i && s.col == j && is_left_head_of(g, anchor, sub)
                };
                if owned {
                    let new_sub = g.clone_cell(sub);
                    g.add_row(new_sub, k);
                    let (rm, cm) = (
                        g.cells[new_sub.0 as usize].row_merge,
                        g.cells[new_sub.0 as usize].col_merge,
                    );
                    // 合并块整体铺满 [x, x+rm) × [j, j+cm)（`:1015-1021`）
                    g.fill_rect(x, j, x + rm, j + cm, new_sub);
                    if sub == anchor {
                        ext_cells[m as usize] = new_sub;
                    }
                }
                // 跳块：`j += sub.getColMerge() - 1` + for 的 ++j（`:1026`）
                j += g.cells[sub.0 as usize].col_merge;
            }
        }
    }

    // ── 第 1 趟收尾：数组已填满，建一次共享分配并分发（`:1027-1029`）──
    // 锚点与每个克隆共用同一条 `Arc<[CellId]>`：内存 O(n)、克隆 O(1)。
    let shared: std::sync::Arc<[CellId]> = ext_cells.into();
    g.set_ext_cells(anchor, std::sync::Arc::clone(&shared));
    for m in 1..count {
        let cl = shared[m as usize];
        if cl.is_some() {
            g.set_ext_cells(cl, std::sync::Arc::clone(&shared));
        }
    }

    // ── 第 2 趟：修 head 指针（`:1030-1056`）──
    for m in 1..count {
        let k = m * rows;
        for i in begin_row..end_row {
            let x = i + k;
            let mut j = 1;
            while j < icol {
                let sub = g.at_pos(i, j);
                if sub.is_none() {
                    j += 1;
                    continue;
                }
                let owned = {
                    let s = &g.cells[sub.0 as usize];
                    s.row == i && s.col == j && is_left_head_of(g, anchor, sub)
                };
                if owned {
                    let new_sub = g.at_pos(x, j);
                    // leftHead：**无条件平移**到 head + k 同列（`:1044-1048`）。
                    // sub != anchor 时链非空（isLeftHeadOf 为真 ⇒ 链上能到锚点）。
                    if sub != anchor {
                        let (hr, hc) = {
                            let h = &g.cells[g.cells[sub.0 as usize].left_head.0 as usize];
                            (h.row, h.col)
                        };
                        let new_head = g.at_pos(hr + k, hc);
                        g.set_cell_left_head(new_sub, new_head)?;
                    }
                    // topHead：**原样共享**（root 除外，`:1049-1051`）；
                    // setCellTopHead 会把克隆登记进 top 的子格列表（模块文档硬约束 1）
                    let top = g.cells[sub.0 as usize].top_head;
                    if top != CellId::ROOT {
                        g.set_cell_top_head(new_sub, top)?;
                    }
                }
                j += g.cells[sub.0 as usize].col_merge;
            }
        }
    }
    Ok(())
}
