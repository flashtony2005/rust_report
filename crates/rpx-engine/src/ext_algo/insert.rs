//! 矩阵插行/插列（附件 I.3；`ExtCellSet.java:809-830` / `:902-926`）。
//!
//! # ⛔ 插列是「部分平移」（读 `Matrix.java:253-281` 的定案，修正 I.10 的近似）
//!
//! `Matrix.insertCols(col, count, begin, end, alignColHead)`：
//! 只平移 `[begin,end]` 带内行；第 0 行仅当 `alignColHead` 为真；
//! **其余行原位不动、新列留空**。实测对拍：`engine_ext` 的 B1/C1/D1 从
//! (1,2)/(1,3)/(1,4) 被挤到 (1,6)/(1,7)/(1,8) —— 带内平移的产物（设计文档
//! §6.3 探针表）。插行（`Matrix.insertRows` 2 参版 `:206-225`）是**全表平移**。
//!
//! # 字段修正扫描的语义（两方向同构）
//!
//! 矩阵层平移后，格对象的 `row`/`col` 字段还停在旧值；扫描**只平移主格自身**
//! （`i == cell.getRow() + count && j == cell.getCol()` 命中锚点新位才 `addRow`），
//! 合并块内的铺满槽位靠 `i = beginRow + rowMerge - 1` 的跳块跳过 ——
//! 「只有主格自身的 row 被平移」是附件 I.3 的原文结论。
//!
//! ⚠ Java 的 for 循环里 `i = beginRow` 之后 `++i`：Rust 的 while 译文在跳块处
//! 置 `i = 跳块终点 + 1`；`continue` 处 `i += 1`。逐行对照过，勿再"优化"。

use crate::arena::CellId;
use crate::heads::HeadState;

/// `_$3(int row, int count)`（`ExtCellSet.java:809-830`）：
/// 插行 = 全表平移 + 字段修正扫描。
pub(crate) fn insert_rows(g: &mut HeadState, row: i32, count: i32) {
    if count < 1 {
        return;
    }
    shift_rows(g, row, count);
    let (irow, icol) = (g.row_size, g.col_size);
    for j in 0..icol {
        let mut i = row + count;
        while i < irow {
            let cell = g.at_pos(i, j);
            if cell.is_none() {
                i += 1;
                continue;
            }
            let mut begin_row = g.cells[cell.0 as usize].row;
            if begin_row < row {
                i += 1; // 插入线以上的格：没被移动，跳过（`:816`）
                continue;
            }
            if i == begin_row + count && j == g.cells[cell.0 as usize].col {
                g.add_row(cell, count); // 只平移主格自身（`:819-821`）
                begin_row += count;
            }
            begin_row += g.cells[cell.0 as usize].row_merge - 1;
            if begin_row <= i {
                i += 1; // 合并块内非锚点槽：跳过（`:821`）
                continue;
            }
            i = begin_row + 1; // 跳块（for 的 `i = beginRow; ++i`）
        }
    }
}

/// `Matrix.insertRows`（`Matrix.java:206-225`）：row 及其后的所有槽位整体下移。
fn shift_rows(g: &mut HeadState, row: i32, count: i32) {
    let all: Vec<((i32, i32), CellId)> = g
        .by_pos
        .drain()
        .map(|((r, c), id)| {
            let r2 = if r >= row { r + count } else { r };
            ((r2, c), id)
        })
        .collect();
    for (pos, id) in all {
        g.by_pos.insert(pos, id);
    }
    g.row_size += count;
}

/// `_$1(col, count, begin, end, alignColHead)`（`ExtCellSet.java:902-926`）+
/// `Matrix.insertCols` 5 参版（`Matrix.java:253-281`）：**部分平移**插列。
pub(crate) fn insert_cols(
    g: &mut HeadState,
    col: i32,
    count: i32,
    begin: i32,
    end: i32,
    align_col_head: bool,
) {
    if count < 1 {
        return;
    }
    // ── 矩阵层：带内行与（可选的）第 0 行平移；带外行原位、新列留空 ──
    let all: Vec<((i32, i32), CellId)> = g
        .by_pos
        .drain()
        .map(|((r, c), id)| {
            let in_band = begin <= r && r <= end;
            let shift = in_band || (r == 0 && align_col_head);
            let c2 = if shift && c >= col { c + count } else { c };
            ((r, c2), id)
        })
        .collect();
    for (pos, id) in all {
        g.by_pos.insert(pos, id);
    }
    g.col_size += count;

    // ── 第 0 行列头对齐：新位置上的格 addCol（`:907-914`）──
    if align_col_head {
        for j in col + count..g.col_size {
            let cell = g.at_pos(0, j);
            if cell.is_some() {
                g.add_col(cell, count);
            }
        }
    }

    // ── 带内行的字段修正扫描（`:916-925`；**含端点 end**）──
    let icol = g.col_size;
    let mut i = begin;
    while i <= end {
        let mut j = col + count;
        while j < icol {
            let cell = g.at_pos(i, j);
            let (crow, ccol) = if cell.is_some() {
                (g.cells[cell.0 as usize].row, g.cells[cell.0 as usize].col)
            } else {
                (i, i32::MIN) // 空槽：走 `< col` 分支跳过
            };
            if cell.is_none() || crow != i || ccol < col {
                j += 1;
                continue;
            }
            let mut begin_col = ccol;
            if j == begin_col + count && crow == i {
                g.add_col(cell, count); // 只平移主格自身
                begin_col += count;
            }
            begin_col += g.cells[cell.0 as usize].col_merge - 1;
            if begin_col <= j {
                j += 1;
                continue;
            }
            j = begin_col + 1; // 跳块
        }
        i += 1;
    }
}
