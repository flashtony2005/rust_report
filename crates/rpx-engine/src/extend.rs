//! extendMode：声明值、运行时生效值与扩展行数口径（M2-15 / 附件 H.4）。
//!
//! 码值来自 `props.rs::def`（`EXTEND_DEFAULT=16 / EXTEND_NONE=17 /
//! EXTEND_VERTICAL=18 / EXTEND_HORIZONTAL=19`），与 `NormalCell` 槽 7 同源。
//!
//! **核心结论（反直觉）**：`16` 不是"不扩展"。产品在 `ExtExtendCell.prepareCalculate`
//! 里按 `leftHead`/`topHead` 拓扑把它**改写成 18/19/17 之一**：
//!
//! ```java
//! public void prepareCalculate(Context ctx, Map<String, String> macros) {
//!     try {
//!         if (this.engine.expression != null) {          // <-- 门禁：只对带表达式的格生效
//!             byte extend = this.getExtensible();
//!             if (16 == extend) {
//!                 ExtCell cell00 = this.engine.cs.getCell00();
//!                 if (this.leftHead == cell00 && this.topHead != cell00) {
//!                     this.setExtensible((byte)19);
//!                 } else if (this.leftHead != cell00 && this.topHead != cell00) {
//!                     this.setExtensible((byte)17);
//!                 } else {
//!                     this.setExtensible((byte)18);
//!                 }
//!             }
//!             this.setCalc(false);
//!         } else { ... }
//! ```
//!
//! ⛔ **上面这段只是两条写入路径之一**（扩展格）。另一条在 `ExtNormalCell.prepareCalculate`
//! 里：**有表达式但不是扩展格**的普通格被**无条件置 17**，跟 heads 拓扑无关：
//!
//! ```java
//! // ExtNormalCell.java:539-541
//! if (this.engine.expression != null) { this.setExtensible((byte)17); }
//! ```
//!
//! 只实现第一条会"因为错的原因得出对的答案" —— 详见 [`CellRole`]。三条合起来的完整规则：
//!
//! | 角色 | 改写规则 | 源码 |
//! |------|----------|------|
//! | 无表达式 | 不改写，保留声明值 | `ExtExtendCell.java:330` / `ExtNormalCell.java:542` |
//! | 扩展格 | 声明 16 → 拓扑；17/18/19 不动 | `ExtExtendCell.java:315-329` |
//! | 普通公式格 | **无条件 → 17** | `ExtNormalCell.java:539-541` |
//!
//! 三条细节都由 `engine_*.expected.json` 实测坐实：**改写只对"带表达式"的格生效**
//! （无表达式的格保留声明值 —— 所以计算态里到处能看到 `extend=16`：`engine_sum` 27 次、
//! `engine_expr` 15 次、`engine_merge` 10 次；把 16 无条件改写成 18 会让这些格全表不符）；
//! **比较的是 `cell00` 这个对象而不是 `null`**，`leftHead` 为 `null` 时 `null == cell00`
//! 为假，落进"非 cell00"一侧（该结论以 `cell00 != null` 为前提；实际恒成立 ——
//! `ExtCellSet.java:148` 把 `_$10` 赋成 `new ExtExtendCell(this,0,0,1,1,…)` 且从不置空）。
//! 本模块用 [`CellId`] 表达同一性，`cell00` 由调用方传入。
//!
//! 算法正文（本项目约定：算法写进 md，代码只留指针）见
//! `docs/m2-engine-design.md` §4.3 与 `docs/format-contract.md` 附件 H.4。

use crate::arena::CellId;

/// extendMode 的四个码值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtendMode {
    /// 16：声明态默认值；运行时会被拓扑改写成 17/18/19（见模块文档）。
    Default16,
    /// 17：不展开 —— 整列收成一个 `List` 落在一格里（`type=list`）。
    None17,
    /// 18：纵向扩展 —— 每个数据行占一格，向下插入 N−1 个新行。
    Vertical18,
    /// 19：水平扩展 —— 每个数据行占一格，向右插入；**并把列结构本身放大 N 倍**
    /// （附件 H.4 补遗：1 列 → 5 列，兄弟列记 `col_merge=5`，"只影响单元格"是错的）。
    Horizontal19,
}

impl ExtendMode {
    /// 全部四个码值，便于遍历与穷举测试。
    pub const ALL: [ExtendMode; 4] = [
        ExtendMode::Default16,
        ExtendMode::None17,
        ExtendMode::Vertical18,
        ExtendMode::Horizontal19,
    ];

    /// 码值（16/17/18/19）。
    pub fn code(self) -> i8 {
        match self {
            ExtendMode::Default16 => 16,
            ExtendMode::None17 => 17,
            ExtendMode::Vertical18 => 18,
            ExtendMode::Horizontal19 => 19,
        }
    }

    /// 由码值还原。`None` 表示不是已知的扩展方式（模板里是 i8，可能被填成别的值）。
    pub fn from_code(code: i8) -> Option<Self> {
        match code {
            16 => Some(ExtendMode::Default16),
            17 => Some(ExtendMode::None17),
            18 => Some(ExtendMode::Vertical18),
            19 => Some(ExtendMode::Horizontal19),
            _ => None,
        }
    }

    /// 是否纵向铺开（插入新行）。
    pub fn extends_vertically(self) -> bool {
        self == ExtendMode::Vertical18
    }

    /// 是否横向铺开（插入新列，并放大列结构）。
    pub fn extends_horizontally(self) -> bool {
        self == ExtendMode::Horizontal19
    }

    /// 是否是"尚未按拓扑改写"的声明态值。
    pub fn is_unresolved_default(self) -> bool {
        self == ExtendMode::Default16
    }

    /// 该格会不会驱动扩展（17 与未改写的 16 都不会）。
    pub fn drives_extension(self) -> bool {
        self.extends_vertically() || self.extends_horizontally()
    }
}

/// 附件 H.4 的拓扑判定（`ExtExtendCell.java:321-327` 的纯粹形式）。
///
/// - `(cell00, 非 cell00)` → 19
/// - `(非 cell00, 非 cell00)` → 17
/// - 其余（含 `(cell00, cell00)` 与 `(非 cell00, cell00)`，以及 heads 为 `null`）→ 18
///
/// ⛔ 这只是**扩展格**那条路径的一半：它只在「格是扩展格 + 带表达式 + 声明 16」时被用到，
/// 门禁与另一条路径见 [`CellRole`] / [`ExtendMode::resolve`]。
pub fn topology_default(left_head: CellId, top_head: CellId, cell00: CellId) -> ExtendMode {
    let left_is_cell00 = left_head == cell00;
    let top_is_cell00 = top_head == cell00;
    if left_is_cell00 && !top_is_cell00 {
        ExtendMode::Horizontal19
    } else if !left_is_cell00 && !top_is_cell00 {
        ExtendMode::None17
    } else {
        ExtendMode::Vertical18
    }
}

/// 角色判定与 `resolve` 在 [`role`] 子模块（本文件已接近 300 行门禁）。
mod role;

pub use role::{
    cell_role, cell_role_of, is_extend_excluded_type, CellRole, EXTEND_EXCLUDED_CELL_TYPES,
};

/// 扩展后的计算态**行数**，口径 = `IReport.getRowCount()`。
///
/// `计算态行数 = 模板行数 + (数据集行数 − 1) × 块高`
///
/// # 口径（踩过一次，别再踩）
///
/// `getRowCount()` 在**模板态**（`ReportDefine.java:359-361`）与**计算态**
/// （`ExtCellSet.java:414-415`）都是 `cellMatrix.getRowSize() - 1`，**两者同一口径**：
/// **不含 0 号脚手架行/列**。而 `ReportDefine2(nRow, nColumn)` 的内部是
/// `new Matrix(++nRow, ++nColumn)`（`ReportDefine2.java:56`），所以**它的第一个参数本身
/// 就已经是这个口径**，不需要再 +1。
///
/// 实测坐实（`rpx-cli print tests/fixtures/engine_sum.rpx`）：
/// `matrix: 7x10 (logical 6x9)` —— `ReportDefine2(6,9)` ↔ 裸矩阵 7 行 ↔ 本函数口径 6 行。
///
/// ⚠ **`rpx_core::ReportDefine.rows` 是裸矩阵行数**（= `getRowSize()`，engine_sum 为 7）。
/// 从解析结果调用本函数时必须先减 1：`calc_row_count(define.rows - 1, …)`。
/// 有 `tests/units_contract.rs` 拿真实夹具钉住这条换算。
///
/// # 参数
///
/// - `template_row_count`：模板行数（= `ReportDefine2` 第一参数 = 模板 `getRowCount()`）。
/// - `dataset_rows`：驱动纵向扩展的数据集行数。
/// - `block_height`：锚点的 `row_merge`（附件 H.7.1：合并跨度不因扩展而收缩，插入的是
///   **整个合并块**）。无合并时为 1，退化成附件 H.4 的 `模板行数 + (数据集行数 − 1)`。
///
/// 六个夹具逐条成立（单测 `row_count_matches_every_fixture` 列了 (模板, 数据, 块高, 实测)）。
/// `dataset_rows <= 0` 无基准（P0 未测空数据集）：按"不插入任何行"处理 —— 这是**约定**不是实测结论。
pub fn calc_row_count(template_row_count: i32, dataset_rows: i32, block_height: i32) -> i32 {
    let extra = (dataset_rows.max(1) - 1) * block_height.max(1);
    template_row_count + extra
}

#[cfg(test)]
mod tests;
