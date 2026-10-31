//! 扩展包络（附件 I.2）与行头判定（附件 I.3）。
//!
//! ⛔ 方向纪律：**行包络沿 `left_subs` 递归、列包络沿 `top_subs` 递归**
//! （`ExtCellSet.java:928-951` / `:1059-1082`）——方向反了带高/带宽必错。
//! 直觉：纵向扩展的作用域由左 head 链决定，横向由上 head 链决定。
//!
//! 四个包络函数都**先取自身坐标再递归求 min/max**：单格无子格时
//! `rows = row_merge`、`cols = col_merge`（附件 I.2 末段）。

use crate::arena::CellId;
use crate::heads::HeadState;

/// 行头行类型的同族集合（`_$2(int):846-857`：`{-91, -95, -94, -90}`）。
/// 与 `heads/grid.rs` 的 `ROW_TYPE_FAMILY` 一致（`-90` 是分组头，附件 J）。
const ROW_HEADER_TYPES: [i8; 4] = [-91, -95, -94, -90];

/// `_$4(cell)`（`:928-939`）：beginRow = min(自身行, 左子格递归 min)。
pub(crate) fn begin_row(g: &HeadState, cell: CellId) -> i32 {
    let me = &g.cells[cell.0 as usize];
    let mut row = me.row;
    for &sub in &me.left_subs {
        let r = begin_row(g, sub);
        if r < row {
            row = r;
        }
    }
    row
}

/// `_$3(ExtCell)`（`:940-951`）：endRow = max(自身行+rowMerge, 左子格递归 max)。
pub(crate) fn end_row(g: &HeadState, cell: CellId) -> i32 {
    let me = &g.cells[cell.0 as usize];
    let mut row = me.row + me.row_merge;
    for &sub in &me.left_subs {
        let r = end_row(g, sub);
        if r > row {
            row = r;
        }
    }
    row
}

/// `_$2(ExtCell)`（`:1059-1070`）：beginCol = min(自身列, 上子格递归 min)。
pub(crate) fn begin_col(g: &HeadState, cell: CellId) -> i32 {
    let me = &g.cells[cell.0 as usize];
    let mut col = me.col;
    for &sub in &me.top_subs {
        let c = begin_col(g, sub);
        if c < col {
            col = c;
        }
    }
    col
}

/// `_$1(ExtCell)`（`:1071-1082`）：endCol = max(自身列+colMerge, 上子格递归 max)。
pub(crate) fn end_col(g: &HeadState, cell: CellId) -> i32 {
    let me = &g.cells[cell.0 as usize];
    let mut col = me.col + me.col_merge;
    for &sub in &me.top_subs {
        let c = end_col(g, sub);
        if c > col {
            col = c;
        }
    }
    col
}

/// `_$2(int row)`（`:846-857`）：行头行判定。
/// `row == 0` → 真；`get(row, 0)` 为空 → 真；否则 rowType ∈ `{-91,-95,-94,-90}` → 真。
/// ⇒「行头行」集合 = {0} ∪ {rowType ∈ 同族}（附件 I.3）。
pub(crate) fn is_row_header(g: &HeadState, row: i32) -> bool {
    if row == 0 {
        return true;
    }
    let cell = g.at_pos(row, 0);
    if cell.is_none() {
        return true;
    }
    ROW_HEADER_TYPES.contains(&g.cells[cell.0 as usize].row_type)
}

/// `_$7()`（`:858-865`）：**第一个**行头行（从 1 起向后扫）；无 → -1。
pub(crate) fn first_header_row(g: &HeadState) -> i32 {
    for i in 1..g.row_size {
        if is_row_header(g, i) {
            return i;
        }
    }
    -1
}

/// `_$6()`（`:867-874`）：**最后一个**行头行（从末尾向前扫）；无 → -1。
pub(crate) fn last_header_row(g: &HeadState) -> i32 {
    for i in (1..g.row_size).rev() {
        if is_row_header(g, i) {
            return i;
        }
    }
    -1
}
