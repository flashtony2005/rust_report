//! 计算态的「对外视图」：与 `expected.json` 的 `calculated` 段一一对应的形状。
//!
//! ⛔ 这一层故意用 `Vec` 线性查找而不是矩阵槽位：它服务于**逐格对拍**（M2-09），
//! 形状要跟夹具一样，而不是跟内部分布一致。真正的矩阵在 [`super::CalcMatrix`]。

use super::SourceRef;
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

/// 计算态网格（`expected.json` 的 `calculated` 段）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CalcGrid {
    /// 数据行数（不含表头行，与 `expected.json` 的 `calculated.rows` 同口径）。
    pub rows: i32,
    /// 数据列数。
    pub cols: i32,
    /// 稀疏存放的格 —— 只收 [`CalcCell::is_emitted`] 为真的格。
    pub cells: Vec<CalcCell>,
}

impl CalcGrid {
    /// 新建空网格。
    pub fn new(rows: i32, cols: i32) -> Self {
        Self { rows, cols, cells: Vec::new() }
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
