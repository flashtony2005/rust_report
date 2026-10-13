//! 计算态的**两张矩阵**与格的地址。
//!
//! # 为什么必须是两张（`docs/m2-engine-design.md` §0.1）
//!
//! Java 的 `ExtCellSet` 同时持有两张矩阵：
//!
//! | Java | 本模块 | 语义 |
//! |------|--------|------|
//! | `_$25` | [`EngineMatrix`] | **引擎矩阵**：模板坐标。扩展期**从不被 insert/改写**，只在阶段 B 填 `srcCurrent` |
//! | `_$26` | [`CalcMatrix`] | **计算矩阵**：会被 `insertRows`/`insertCols`/整块改写 |
//!
//! `expected.json` 的 `source` 段读的是**引擎矩阵**（`ExtCellSet.getSource:623-629`），
//! 所以「计算态第 4 行」的 `source` 描述的是**模板第 (4,c) 格**，往往不是这个值所属的实例。
//!
//! 由此得到一条可断言的契约（6/6 夹具 `fn=fp=0`，见 [`EngineMatrix::get`]）：
//! `source != null` ⟺ `1 <= r <= template_rows && 1 <= c <= template_cols`。
//!
//! ⛔ 把两张矩阵合并成一张「省事」会让除锚点外的 `source` 全部错位 —— 见 §0.1 末段的警告。
//! 这也是 `engine_ext` 42 格里 30 格、`engine_pager` 124 格里 114 格没有 `source` 的根因。
//!
//! # 哨兵约定
//!
//! Java 的 `null` → [`CellId::NONE`]；Java 的 `cell00`（矩阵原点格）→ [`CellId::ROOT`]。
//! Java 用对象引用的**同一性**（`==`）表达「同一个格」（`MainCell.java:25` 的 `leftHead == cell00`），
//! Rust 用整数相等表达 —— 语义等价且天然 `Copy`。
//! ⚠ 已知会踩的雷：`leftHead == null` 在 Java 里结果是 **false**（落进「非 cell00」一侧 → 17），
//! 所以 [`CellId::NONE`] **不等于** [`CellId::ROOT`]。

mod calc_grid;
mod calc_matrix;
mod engine_matrix;
mod source_ref;

#[cfg(test)]
mod tests;

pub use calc_grid::{CalcCell, CalcGrid, SubValue};
pub use calc_matrix::CalcMatrix;
pub use engine_matrix::EngineMatrix;
pub use source_ref::SourceRef;

/// 计算态里一个格的索引（arena 下标）。`NONE` 对应 Java 的 `null`。
///
/// 一个 `CellId` 在**计算矩阵**里可以同时占据多个槽位 —— 合并区「铺满」时整个矩形
/// 的槽位都指向同一个 id（`ExtCellSet.java:177-183`、`_$1(...):706-712`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellId(pub u32);

impl CellId {
    /// Java `null` 的替身。**与 [`CellId::ROOT`] 不相等**（见模块文档末段）。
    pub const NONE: CellId = CellId(u32::MAX);
    /// 矩阵原点格 `cell00`（`ExtCellSet.getCell00()`）；即 arena 的第 0 个格。
    pub const ROOT: CellId = CellId(0);

    /// 是否是有效索引（非 `NONE`）。
    pub fn is_some(self) -> bool {
        self != CellId::NONE
    }

    /// 是否是 `NONE`（Java 的 `null`）。
    pub fn is_none(self) -> bool {
        self == CellId::NONE
    }

    /// arena 下标；`NONE` 返回 `None`。
    pub fn index(self) -> Option<usize> {
        if self.is_some() {
            Some(self.0 as usize)
        } else {
            None
        }
    }
}
