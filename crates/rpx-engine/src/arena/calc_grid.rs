//! 计算态的「对外视图」：与 `expected.json` 的 `calculated` 段一一对应的形状。
//!
//! ⛔ 这一层故意用 `Vec` 线性查找而不是矩阵槽位：它服务于**逐格对拍**（M2-09），
//! 形状要跟夹具一样，而不是跟内部分布一致。真正的矩阵在 [`super::CalcMatrix`]。

use super::SourceRef;
use crate::pager::Pagination;
use crate::value::Value;

/// `source.sub_values[]` 的一项（`CalcJson.java:105-112`）：一个扩展实例自己的
/// 计算态坐标与值。
///
/// ⛔ 坐标是**计算态**坐标（`sub.getRow()/getCol()`），不是模板出处 —— 与
/// [`super::SourceRef`] 的 `src_row/src_col` 是两套坐标，别混。
#[derive(Debug, Clone, PartialEq)]
pub struct SubValue {
    /// 该实例的计算态行号。
    pub r: i32,
    /// 该实例的计算态列号。
    pub c: i32,
    /// 该实例的值（`sub.getValue(false)`）。
    pub value: Value,
}

impl SubValue {
    /// 一个实例项。
    pub fn new(r: i32, c: i32, value: Value) -> Self {
        Self { r, c, value }
    }
}

/// 计算态的一格。
#[derive(Debug, Clone, PartialEq)]
pub struct CalcCell {
    /// 计算态行号（1 起）。
    pub row: i32,
    /// 计算态列号（1 起）。
    pub col: i32,
    /// 求值结果。未求值时是 [`Value::Null`]（Java 的 `cellValue == null`）。
    pub value: Value,
    /// 显示值；`None` 表示夹具没写这个键。
    ///
    /// 探针只在 `disp_value != String.valueOf(value)` 时才写（`CalcJson.java:71-73`），
    /// 所以它**不是** `value` 的格式化结果，而是一个独立的覆盖项。
    /// ⚠ 早先（只有 7 个夹具时）我误判它是"死键"；第 8 个夹具 `engine_disp` 证伪：
    /// 它有 7 格带 `disp_value`，其中 **1 格的 `value` 是 `null`**（`type` = `"null"`，
    /// `disp_value` = `"仅显示值"`）—— 正是这一格把 [`CalcCell::is_emitted`] 的判定式钉死。
    pub disp_value: Option<String>,
    /// 模板出处；`None` 表示该格没有 `source` 键。
    pub src: Option<SourceRef>,
    /// 扩展产生的子格（`sub_values`）。长度 `<= 1` 时夹具不落盘
    /// （`CalcJson.java:84-97` 的 `if (subs > 1)`）。
    ///
    /// ⛔ 数据源是**引擎矩阵**在 `(r, c)` 上那一格的 `ext_cells`
    /// （`CalcJson.java:104-113` 读的是 `src.getExtCell(i)`，而 `src = ecs.getSource(r,c)`
    /// 取的是 `_$25`）—— 同一个 `(r, c)` 上，格本体来自计算矩阵、`source`/`sub_values`
    /// 来自引擎矩阵，这是本项目最容易混的一处（见 [`super::EngineMatrix`] 模块文档）。
    pub sub_values: Vec<SubValue>,
    /// `left_head` 的**模板出处** `{src_row, src_col}`；没有 head 时夹具省略该键。
    ///
    /// 出处是 `CalcJson.headJson`：它读 `head.getSourceRow()/getSourceCol()` ——
    /// 是 **head 目标那一格自己的模板坐标**，不是它在计算态里的 `getRow()/getCol()`
    /// （167 个被引用的 head 目标里两者有 149 个不同）。所以这里直接存模板坐标对，
    /// 而不是存 `CellId`（存 id 会在导出期被迫做一次坐标换算，那正是要避免的歧义）。
    pub left_head: Option<(i32, i32)>,
    /// `top_head` 的模板出处。语义同 [`Self::left_head`]。
    pub top_head: Option<(i32, i32)>,
}

impl CalcCell {
    /// 一个没有来源信息、没有显示值、没有 head 的格。
    pub fn bare(row: i32, col: i32, value: Value) -> Self {
        Self {
            row,
            col,
            value,
            disp_value: None,
            src: None,
            sub_values: Vec::new(),
            left_head: None,
            top_head: None,
        }
    }

    /// 一个"只有显示值"的格 —— `value` 为 null 但 `disp_value` 非空（`engine_disp` 的形态）。
    pub fn display_only(row: i32, col: i32, disp_value: String) -> Self {
        Self {
            row,
            col,
            value: Value::Null,
            disp_value: Some(disp_value),
            src: None,
            sub_values: Vec::new(),
            left_head: None,
            top_head: None,
        }
    }

    /// 补上 head 出处（导出期的最后一步）。
    pub fn with_heads(mut self, left: Option<(i32, i32)>, top: Option<(i32, i32)>) -> Self {
        self.left_head = left;
        self.top_head = top;
        self
    }

    /// 该格是否参与合并（无 `source` 一律视为没合并）。
    pub fn is_merged(&self) -> bool {
        self.src.is_some_and(|s| s.is_merged())
    }

    /// 该格是否落盘到 `expected.json` 的 `cells` 数组。
    ///
    /// 探针的判定是 `if (value == null && dispValue == null) continue;`
    /// （`CalcJson.java:51-53`）—— **两个都为 null 才跳过**，所以 `cells` 是**稀疏**的：
    /// 「坐标不在 `cells` 里」= 「该格为空」，**不是**「断言失败」。
    /// 逐格对拍必须先按 `(r,c)` 建索引，不能按下标对位。
    ///
    /// ⚠ 判定式是 `||` 不是 `&&`：`value` 为 null 但 `disp_value` 非空的格 **会** 落盘
    /// （`engine_disp` 的 `(2,3)`：`type="null"`、`disp_value="仅显示值"`）。
    pub fn is_emitted(&self) -> bool {
        !self.value.is_null() || self.disp_value.is_some()
    }
}

/// `merged.cells[]` 的一项（`CalcMergeJson.java:51-58`）。
///
/// ⛔ 与 [`CalcCell::is_merged`] **不是同一件事**：那个读的是 `source` 段（引擎矩阵
/// `_$25` 的跨度），本结构读的是**计算矩阵** `_$26` 在合并趟之后的值 —— 两趟合并只改
/// 计算矩阵，所以 `engine_samemerge` 的跨度**只**在这里可见。
///
/// ⛔ 本段**不受** `cells` 稀疏规则约束（`CalcMergeJson.java:42-50` 只跳 `rm<=1 && cm<=1`，
/// 不跳「值为空」）⇒ 锚点值可能为 [`Value::Null`]，此时 `calculated.cells` 里没有它。
///
/// # 坐标口径：`CalcGrid` 里是**全局**，emit 产出才是**页内**（M5-02）
///
/// [`CalcGrid::merged`] 由 `calc/run.rs::export_merged` 按扫描序填**全局**坐标
/// （`page` 是占位 `1`）—— `rpx-server` 的快照映射（`snapshot/geom.rs::merge_data`
/// 的矩形槽集不变式 + `paged.rs` 的跨页拆分）吃的就是这一份。**页内**视图
/// （真页号 + 页局部坐标 + 页内跨度）由 `emit::merged_section` 按
/// `grid.pagination` 投影产出，对拍真值（`CalcMergeJson` 读页格）要的是后者。
#[derive(Debug, Clone, PartialEq)]
pub struct MergedCell {
    /// 页号（1 起）。
    ///
    /// [`CalcGrid::merged`] 里恒为占位 `1`；emit 按 `grid.pagination` 重写为真页号
    /// （`cross_emp_bonus`：标题 9 列合并第 1 页 8 槽 + 第 2 页 1 槽因 `1×1` 被跳过）。
    pub page: i32,
    /// 行号（`CalcGrid` 里 = 全局；emit 产出 = 页内）。
    pub r: i32,
    /// 列号（`CalcGrid` 里 = 全局；emit 产出 = 页内）。
    pub c: i32,
    /// 该槽上那一格的**计算态值**（延续槽取锚点的值）。
    pub value: Value,
    /// 纵向跨度（`getRowMerge()`；`CalcGrid` 里 = 整段，emit 产出 = 页内交矩形）。
    pub row_merge: i32,
    /// 横向跨度（`getColMerge()`；`CalcGrid` 里 = 整段，emit 产出 = 页内交矩形）。
    pub col_merge: i32,
}

impl MergedCell {
    /// 一项。
    pub fn new(page: i32, r: i32, c: i32, value: Value, row_merge: i32, col_merge: i32) -> Self {
        Self {
            page,
            r,
            c,
            value,
            row_merge,
            col_merge,
        }
    }
}

/// 计算态网格（`expected.json` 的 `calculated` 段 + `merged` 段 + `pagination` 段）。
#[derive(Debug, Clone, PartialEq)]
pub struct CalcGrid {
    /// 数据行数（不含表头行，与 `expected.json` 的 `calculated.rows` 同口径）。
    pub rows: i32,
    /// 数据列数。
    pub cols: i32,
    /// 稀疏存放的格 —— 只收 [`CalcCell::is_emitted`] 为真的格。
    pub cells: Vec<CalcCell>,
    /// `merged.cells`：合并趟之后的**计算矩阵**上所有 `rm>1 || cm>1` 的槽
    /// （含延续槽 —— 它们导出锚点的整段跨度，见 [`MergedCell`]）。
    /// **全局坐标**（M5-02）：服务面快照（`rpx-server` 的 `merge_data` +
    /// 跨页拆分）直接用这份全局视图；页内投影在 [`Self::merged_pages`]。
    pub merged: Vec<MergedCell>,
    /// `merged.cells` 的**页内视图**（M5-02，对拍真值的形状）：
    /// 页局部坐标 + 页内跨度（真页号）。由 `calc/run.rs` 分页后经
    /// [`crate::calc::merged_view`] 填充；未跑分页器时为空（emit 回落
    /// [`Self::merged`]）。⚠ `/render` 会用**模板真值口径**覆盖
    /// [`Self::pagination`]，但**不**重算本字段（服务面快照不读它，
    /// 只读 [`Self::merged`]）—— 它只服务夹具对拍（探针口径）。
    pub merged_pages: Vec<MergedCell>,
    /// `merged.page_count`（`CalcMergeJson.java:63` → `PageBuilder.getPageCount()`）。
    ///
    /// 由 [`crate::pager`] 算出（[`Self::pagination`] 的 `page_count`）；未跑分页器时恒 1。
    pub page_count: i32,
    /// 计算态**每行行高**（索引 0 ↔ 网格第 1 行），单位是 [`Self::unit`]。
    ///
    /// 由 `calculate()` 先按扩展映射铺开模板值，再由 `calcProperties` 行高表达式覆盖。
    /// `engine_rowheight` 的 17 项计算值逐值对拍产品夹具（`tests/engine_rowheight_pager.rs`）。
    pub row_heights: Vec<f32>,
    /// 计算态**每列列宽**（索引 0 ↔ 网格第 1 列），单位是 [`Self::unit`]。
    pub col_widths: Vec<f32>,
    /// 计算态每行的 `rowType`（`-90` = 分组头；`-91` = 普通）。
    pub row_types: Vec<i8>,
    /// 计算态每行是否可见（模板值由扩展行头继承，属性 key 2 可覆盖）。
    pub row_visible: Vec<bool>,
    /// 每行是否在其后强制分页（key 13 / RowCell.breakPage）。
    pub row_break_page: Vec<bool>,
    /// 每行是否触发分栏（key 15 / RowCell.breakColumn）。
    pub row_break_column: Vec<bool>,
    /// 每行是否自动调整高度（key 7 / RowCell.adjustRowH）。
    pub row_adjust_height: Vec<bool>,
    /// 计算态每列是否可见（key 12 / ColCell.colVisible）。
    pub col_visible: Vec<bool>,
    /// 每列是否在其后强制分页（key 13 / ColCell.breakPage）。
    pub col_break_page: Vec<bool>,
    /// 每列是否触发分栏（key 15 / ColCell.breakColumn）。
    pub col_break_column: Vec<bool>,
    /// 报表单位（`ReportDefine.scalars.unit`；`1` = mm）。
    pub unit: i8,
    /// 分页结果。`None` = 未跑分页器（手搭的网格）—— 导出期写 `pagination: null`，
    /// 对拍器据此报 `Missing`，**不静默**。
    pub pagination: Option<Pagination>,
}

impl Default for CalcGrid {
    fn default() -> Self {
        CalcGrid::new(0, 0)
    }
}

impl CalcGrid {
    /// 新建空网格（单页、无几何）。
    pub fn new(rows: i32, cols: i32) -> Self {
        Self {
            rows,
            cols,
            cells: Vec::new(),
            merged: Vec::new(),
            merged_pages: Vec::new(),
            page_count: 1,
            row_heights: Vec::new(),
            col_widths: Vec::new(),
            row_types: Vec::new(),
            row_visible: Vec::new(),
            row_break_page: Vec::new(),
            row_break_column: Vec::new(),
            row_adjust_height: Vec::new(),
            col_visible: Vec::new(),
            col_break_page: Vec::new(),
            col_break_column: Vec::new(),
            unit: 1,
            pagination: None,
        }
    }

    /// 按 (行, 列) 取格；坐标不在（稀疏）数组里返回 `None`（= 该格为空）。
    pub fn get(&self, row: i32, col: i32) -> Option<&CalcCell> {
        self.cells.iter().find(|c| c.row == row && c.col == col)
    }

    /// 所有参与合并的格 —— 对齐 `expected.json` 的 `merged.cells` 段。
    ///
    /// ⚠ 该段的语义要按 `spec-m2.md` §7 读：`merged` 在**全部**夹具里都非 `null`
    /// （错误型为 `null`），`cells` 非空只出现在 `engine_ext`(35)/`engine_merge`(12)/
    /// `engine_samemerge`(4)。⇒ **不能用「`merged` 非空」筛 merge 夹具**，也不能
    /// 反过来把本方法的结果当「发生过 `mergeSame`」的证据。
    pub fn merged_cells(&self) -> Vec<&CalcCell> {
        self.cells.iter().filter(|c| c.is_merged()).collect()
    }
}
