//! 引擎矩阵（Java `ExtCellSet._$25`）：**模板坐标、扩展期只读**。

use super::SourceRef;

/// 引擎矩阵。每一格持有一个 [`SourceRef`]（Java 的 `srcCurrent`），
/// 由阶段 B 写入，**扩展期不再改动**。
///
/// 坐标是**模板态 1 基坐标**（不含 0 号脚手架行/列 —— 脚手架在 `_$25` 里存在但不是
/// `NormalCellEngine`，探针的 `getSource` 取不到它，故本结构不建模）。
/// 行/列数的口径与 `expected.json` 的 `template_rows`/`template_cols` 一致
/// （即 Java 的 `getRowCount()`/`getColCount()`，= 矩阵尺寸 − 1）。
///
/// # 为什么它是只读的
///
/// `_$25` 只在阶段 B 被填 `srcCurrent`；阶段 C 的扩展只对 `_$26` 做
/// `insertRows`/`insertCols`（`ExtCellSet.java:810,881,903`）。
/// 这正是契约 [`EngineMatrix::get`] 的来源，也是 [`super::CalcMatrix`] 必须单独存在的原因。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EngineMatrix {
    /// 模板数据行数（= `getRowCount()`）。
    rows: i32,
    /// 模板数据列数（= `getColCount()`）。
    cols: i32,
    /// 按 `(row-1) * cols + (col-1)` 索引；`None` = 该模板格没有 `srcCurrent`。
    slots: Vec<Option<SourceRef>>,
}

impl EngineMatrix {
    /// 建一张空的模板尺寸矩阵（所有 `srcCurrent` 为 `None`）。
    pub fn new(rows: i32, cols: i32) -> Self {
        let rows = rows.max(0);
        let cols = cols.max(0);
        Self { rows, cols, slots: vec![None; (rows * cols) as usize] }
    }

    /// 模板数据行数。
    pub fn rows(&self) -> i32 {
        self.rows
    }

    /// 模板数据列数。
    pub fn cols(&self) -> i32 {
        self.cols
    }

    /// 坐标是否落在模板尺寸内（1 基、闭区间）。
    ///
    /// 这条谓词就是 `source != null` 的判定式 —— 见 [`SourceRef`] 的文档。
    /// ⚠ 6/6 夹具 `fn=fp=0` 成立的前提是调用方只问 `r >= 1 && c >= 1`
    /// （`(0,0)` 在 Java 里走 `ExtCellSet.java:616` 的特例，不经过本矩阵）。
    pub fn is_within_template(&self, row: i32, col: i32) -> bool {
        row >= 1 && row <= self.rows && col >= 1 && col <= self.cols
    }

    /// 取模板格 (row, col) 的出处；**越出模板尺寸返回 `None`**。
    ///
    /// 这是 `expected.json` 里 `source` 键有无的判定 —— 越界即为扩展出来的格，
    /// 没有 `source`。六个网格夹具上 `fn`（界内却无 `source`）与 `fp`（界外却有 `source`）
    /// 均为 0。
    pub fn get(&self, row: i32, col: i32) -> Option<SourceRef> {
        if !self.is_within_template(row, col) {
            return None;
        }
        self.slots[((row - 1) * self.cols + (col - 1)) as usize]
    }

    /// 阶段 B 写入某模板格的出处。越界忽略（模板尺寸在阶段 A 之后不再变）。
    pub fn set(&mut self, row: i32, col: i32, src: SourceRef) {
        if !self.is_within_template(row, col) {
            return;
        }
        self.slots[((row - 1) * self.cols + (col - 1)) as usize] = Some(src);
    }

    /// 已填出处的格数（调试/断言用）。
    pub fn filled_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
}
