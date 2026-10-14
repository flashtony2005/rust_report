//! 一格在**模板态**的出处，对应 `expected.json` 的 `source` 块。

use crate::extend::ExtendMode;

/// 一格在模板态的出处，对应 `expected.json` 的 `source` 块。
///
/// `expected.json` 里 `source` 是**可选键**（`engine_ext` 42 格中 30 格没有、
/// `engine_pager` 124 格中 114 格没有），所以承载它的字段一律是 `Option`。
///
/// # 什么时候有 `source`（已核实，6/6 夹具 `fn=fp=0`）
///
/// 探针读的是 `ecs.getSource(r, c)` → `ExtCellSet._$25.get(r,c).srcCurrent`，
/// 而 `_$25` 是**扩展前的引擎矩阵、扩展过程从不写它**（只有 `_$26` 被
/// `insertRows/insertCols`，`ExtCellSet.java:810,881,903`）。因此在计算态里：
///
/// `source != null` ⟺ `1 <= r <= 模板行数 && 1 <= c <= 模板列数`
/// （尺寸取 `getRowCount()` 口径，即 `expected.json` 的 `template_rows`/`template_cols`；
/// 另有 `(0,0)` 特例 —— `ExtCellSet.java:616` 让它返回 `cell00`）。
///
/// ⛔ 所以 `source` **不是**「这一格的出处」，而是「这一格坐标上**模板**那一格」。
/// 例：`engine_ext` 计算态 `(4,1)` 的值来自"第 4 行克隆"，但 `source` 给的是模板 `(4,1)`
/// 的 19 锚点。查错矩阵就会在除锚点外的每一格上错位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRef {
    /// 模板态行号（1 起）。
    pub src_row: i32,
    /// 模板态列号（1 起）。
    pub src_col: i32,
    /// 纵向合并跨度，`>= 1`。1 表示没合并。
    pub row_merge: i32,
    /// 横向合并跨度，`>= 1`。
    pub col_merge: i32,
    /// 运行时**生效**的扩展方式（不是模板声明值 —— 声明 16 会被改写成 17/18/19，附件 H.4）。
    pub extend: ExtendMode,
    /// `src.extCellSize()`（`ExtExtendCell.java:126-128`）：`ext_cells` 的长度，
    /// `None` 视作 **1**（夹具 `source.sub_cells` 键）。
    ///
    /// ⛔ 它读的是**引擎矩阵那一格**的实例数组（`CalcJson.java:100`），不是计算态格的。
    /// 计数放在这里而不是查表：`SourceRef` 是 `Copy`，而 `expected.json` 的
    /// `sub_cells` 是纯计数；逐个实例的 `(r, c, value)` 由
    /// [`super::CalcCell::sub_values`] 承载。
    pub sub_cells: i32,
}

impl SourceRef {
    /// 无合并、给定扩展方式的出处（`sub_cells = 1`，即没有扩展实例）。
    pub fn new(src_row: i32, src_col: i32, extend: ExtendMode) -> Self {
        Self {
            src_row,
            src_col,
            row_merge: 1,
            col_merge: 1,
            extend,
            sub_cells: 1,
        }
    }

    /// 是否参与合并（任一方向跨度 `> 1`）。
    pub fn is_merged(&self) -> bool {
        self.row_merge > 1 || self.col_merge > 1
    }
}
