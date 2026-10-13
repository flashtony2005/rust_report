//! 计算矩阵（Java `ExtCellSet._$26`）：会被 `insertRows`/`insertCols`/整块改写的矩阵。

use super::{CalcCell, CalcGrid, CellId};

/// 计算矩阵。**槽位网格 + 格数据 arena** 两段式：
///
/// * `slots` 是 `rows × cols` 的槽位表，每个槽持一个 [`CellId`]；
/// * `cells` 是格的 arena，`cells[id.0]` 就是那个格的数据。
///
/// 这样"合并区铺满"只需把整个矩形的槽位写成**同一个 `CellId`**
/// （Java 是把同一个对象引用塞进每个槽，`_$26.set(i,j,head)`，`ExtCellSet.java:706-712`），
/// 而"插入行/列"只需搬槽位、不动格数据 —— 与 Java 的 `Matrix.insertRows/insertCols` 同构。
///
/// # 坐标口径（⚠ 易错）
///
/// 矩阵坐标含 **0 号脚手架行/列**：`row ∈ 0..rows`、`col ∈ 0..cols`，
/// `(0,0)` 就是 `cell00`（[`CellId::ROOT`]）。
/// 而 Java 的 `getRowCount()` / `expected.json` 的 `rows` 是**数据格数** = 尺寸 − 1。
/// 两者差 1，别混：本结构的 [`CalcMatrix::new`] 收的是**含脚手架的总数**，
/// 与 `rpx_core::ReportDefine.rows/cols` 同口径。
#[derive(Debug, Clone, PartialEq)]
pub struct CalcMatrix {
    /// 总行数（含 0 号脚手架行）。
    rows: i32,
    /// 总列数（含 0 号脚手架列）。
    cols: i32,
    /// 槽位表，按 `row * cols + col` 索引。
    slots: Vec<CellId>,
    /// 格 arena。
    cells: Vec<CalcCell>,
}

impl CalcMatrix {
    /// 建一张全空的矩阵。`rows`/`cols` 是**含 0 号脚手架**的总数
    /// （= `rpx_core::ReportDefine.rows/cols`，= Java 的 `getRowSize()/getColSize()`）。
    pub fn new(rows: i32, cols: i32) -> Self {
        let rows = rows.max(0);
        let cols = cols.max(0);
        Self {
            rows,
            cols,
            slots: vec![CellId::NONE; (rows * cols) as usize],
            cells: Vec::new(),
        }
    }

    /// 总行数（含脚手架）。
    pub fn row_size(&self) -> i32 {
        self.rows
    }

    /// 总列数（含脚手架）。
    pub fn col_size(&self) -> i32 {
        self.cols
    }

    /// Java `getRowCount()` 口径的数据行数 = 总行数 − 1。
    pub fn row_count(&self) -> i32 {
        self.rows - 1
    }

    /// Java `getColCount()` 口径的数据列数 = 总列数 − 1。
    pub fn col_count(&self) -> i32 {
        self.cols - 1
    }

    /// 坐标是否在矩阵内。
    pub fn contains(&self, row: i32, col: i32) -> bool {
        row >= 0 && row < self.rows && col >= 0 && col < self.cols
    }

    /// 取槽位；越界返回 [`CellId::NONE`]（Java 越界同样是 `null`）。
    pub fn get(&self, row: i32, col: i32) -> CellId {
        if !self.contains(row, col) {
            return CellId::NONE;
        }
        self.slots[(row * self.cols + col) as usize]
    }

    /// 写槽位；越界忽略。
    pub fn set(&mut self, row: i32, col: i32, id: CellId) {
        if !self.contains(row, col) {
            return;
        }
        let at = (row * self.cols + col) as usize;
        self.slots[at] = id;
    }

    /// 把**半开矩形** `[begin_row, end_row) × [begin_col, end_col)` 的每个槽写成同一个 id。
    ///
    /// 对齐 `ExtCellSet._$1(startRow, startCol, endRow, endCol, head):706-712`。
    /// 合并区、扩展时整块复制、`expand*Head` 的重铺都走这一条。
    pub fn fill_rect(&mut self, begin_row: i32, begin_col: i32, end_row: i32, end_col: i32, id: CellId) {
        let r0 = begin_row.max(0);
        let c0 = begin_col.max(0);
        for r in r0..end_row.min(self.rows) {
            for c in c0..end_col.min(self.cols) {
                let at = (r * self.cols + c) as usize;
                self.slots[at] = id;
            }
        }
    }

    /// 把一格放进 arena，返回它的 id。
    pub fn push_cell(&mut self, cell: CalcCell) -> CellId {
        let id = CellId(self.cells.len() as u32);
        self.cells.push(cell);
        id
    }

    /// 按 id 取格；`NONE` 或越界返回 `None`。
    pub fn cell(&self, id: CellId) -> Option<&CalcCell> {
        self.cells.get(id.index()?)
    }

    /// 按 id 取格的**可变**引用。
    pub fn cell_mut(&mut self, id: CellId) -> Option<&mut CalcCell> {
        let at = id.index()?;
        self.cells.get_mut(at)
    }

    /// 在 `row` 前插入 `count` 个空行（新行占据 `[row, row+count)`）。
    ///
    /// 对齐 `Matrix.insertRows`（`ExtCellSet._$3(row,count):809-830` 的第一句）。
    /// ⛔ 本方法**只搬槽位**，没有 Java `_$3` 里"把主格自身的 `row` 平移 `count`"
    /// 和 `_$16 += count` 的副作用 —— 那部分属扩展本体（§12.1 第 8 步）。
    pub fn insert_rows(&mut self, row: i32, count: i32) {
        if count <= 0 || row < 0 || row > self.rows {
            return;
        }
        let at = (row * self.cols) as usize;
        let blanks = vec![CellId::NONE; (count * self.cols) as usize];
        self.slots.splice(at..at, blanks);
        self.rows += count;
    }

    /// 在 `col` 前插入 `count` 个空列（新列占据 `[col, col+count)`）。
    ///
    /// 对齐 `Matrix.insertCols`。跨行插入，故需要整表重建；矩阵规模在本批夹具里很小。
    pub fn insert_cols(&mut self, col: i32, count: i32) {
        if count <= 0 || col < 0 || col > self.cols {
            return;
        }
        let (old_cols, ins) = (self.cols, count as usize);
        let head = col as usize;
        let mut out = Vec::with_capacity((self.rows * (old_cols + count)) as usize);
        for r in 0..self.rows {
            let base = (r * old_cols) as usize;
            let row = &self.slots[base..base + old_cols as usize];
            out.extend_from_slice(&row[..head]);
            out.extend_from_slice(&vec![CellId::NONE; ins]);
            out.extend_from_slice(&row[head..]);
        }
        self.slots = out;
        self.cols += count;
    }

    /// 导出为对拍视图；**只收非空格**（`value` 为 null 的格不落盘，`CalcJson.java:51-53`）。
    ///
    /// 每个格的数据取自 arena（合并块的整个矩形会导出成多条同数据的记录 ——
    /// 这与夹具一致：夹具的 `merged` 段就是"被合并的格都报整块跨度"，附件 H.7）。
    pub fn to_grid(&self) -> CalcGrid {
        let mut grid = CalcGrid::new(self.row_count(), self.col_count());
        for r in 1..self.rows {
            for c in 1..self.cols {
                let id = self.get(r, c);
                let Some(cell) = self.cell(id) else { continue };
                if !cell.is_emitted() {
                    continue;
                }
                let mut out = cell.clone();
                out.row = r;
                out.col = c;
                grid.cells.push(out);
            }
        }
        grid
    }
}
