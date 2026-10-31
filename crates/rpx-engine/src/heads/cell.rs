//! [`HeadCell`]：一个格在 head 计算 + 求值期所需的全部属性（自 `grid.rs` 拆出）。
//!
//! 对应 Java 的 `ExtNormalCell`/`ExtExtendCell` 对象本身 —— 产品把「值、head 链、
//! 合并跨度、扩展实例数组」全塞在同一个对象里，所以这里也**不拆**：扩展克隆
//! （`clone_cell`）必须把值一起带走，值分家就会在扩展后与 `CellId` 失步。

use std::sync::Arc;

use crate::arena::CellId;
use crate::value::Value;

use super::runtime::CellRuntime;

/// 一个格在 head 计算期所需的全部属性。
#[derive(Debug, Clone)]
pub struct HeadCell {
    /// 计算态行号（1 起；`cell00` 为 0）。
    pub row: i32,
    /// 计算态列号。
    pub col: i32,
    /// 引擎出处行（`engine.sourceRow`）—— **模板坐标**，克隆/平移**不**改它。
    ///
    /// ⛔ 与 [`Self::row`] 是**两套坐标**，这是本 crate 最容易混的一处：
    /// 扩展会把格搬到别的计算态槽位（`addRow`/`addCol`），但 `sourceRow` 永远是
    /// 它出生时那一格（`ExtNormalCell.java:127-128` 构造时写死、之后无人改）。
    /// 用处有三：① [`crate::expr::EvalCtx::source_of`]（相对引用的 `getSource()`）；
    /// ② 夹具 `left_head`/`top_head` 的 `{src_row, src_col}` 就是这个值
    /// （`CalcJson.headJson` 读 `getSourceRow()/getSourceCol()`）；
    /// ③ 判"是不是引擎矩阵本体格"（`getSource() == this`）。
    pub src_row: i32,
    /// 引擎出处列（`engine.sourceCol`）。语义见 [`Self::src_row`]。
    pub src_col: i32,
    /// 生效 extendMode（16..19）。必须是阶段 B `checkExtend` **改写后**的值 ——
    /// head 命中分支判的就是改写值（`_$3` 判 18、`_$2` 判 19）。
    pub extend: i8,
    /// 所属计算行的 `rowType`（克隆行继承锚点模板行的类型）。
    pub row_type: i8,
    /// 纵向跨度（`containRow` 的宽度）。
    ///
    /// ⛔ 这是**计算矩阵** `_$26` 侧的值（`ExtNormalCell.rowMerge`，`:235-237`）。
    /// 合并两趟会改写它；**引擎矩阵** `_$25` 侧另存一份（见 [`Self::engine_row_merge`]）。
    pub row_merge: i32,
    /// 横向跨度（`containCol` 的宽度）。语义与坑同 [`Self::row_merge`]。
    pub col_merge: i32,
    /// **引擎矩阵侧**（`_$25`）的纵向跨度 —— `NormalCellEngine.rowMerge`。
    ///
    /// 产品在构造时把 `_$25` 填成 `extCell.engine`（`ExtCellSet.java:190`）——
    /// 与 `_$26` 里的 `ExtNormalCell` 是**两个对象**，各有一份 `rowMerge` 字段
    /// （`ExtNormalCell.java:235-237` vs `NormalCellEngine`）。所以合并两趟只动 `_$26`，
    /// `getSource(r,c).getRowMerge()` 读到的仍是合并前的值。
    ///
    /// 夹具铁证（`engine_samemerge` 的 `(2,1)`）：`merged.cells` 报 `row_merge=2`
    /// 而同一格的 `source.row_merge` 报 `1`。
    ///
    /// 冻结时机见 [`super::grid::HeadState::freeze_engine_spans`]（两趟合并**之前**）。
    pub engine_row_merge: i32,
    /// 引擎矩阵侧的横向跨度。语义同 [`Self::engine_row_merge`]。
    pub engine_col_merge: i32,
    /// 列头格的 `colType`（`IColCell.getColType()`）。**只有 `row == 0` 的列头格有意义**，
    /// 其余格保持 [`rpx_core::props::def::COL_TYPE`]。
    ///
    /// 合并的「同列类型」判据读它（`ExtCellSet.java:1959/1988/2012/2036`、
    /// `:2123/2145` 的 `getColCell(j).getColType()`）。
    pub col_type: i8,
    /// `getMergeSame()`（`ExtNormalCell.java:2005` → 模型层 `NormalCell._$4`）：
    /// **97=关 / 98=纵向 / 99=横向 / 100=双向**（`props::def::MERGE_SAME`）。
    pub merge_same: i8,
    /// `getMergeNull()`（模型层 `NormalCell._$3`）：
    /// **113=关 / 114=UP / 115=LEFT / 116=DOWN / 117=RIGHT**。
    pub merge_null: i8,
    /// `getMergeSameMode()`（模型层 `NormalCell._$2`）：**118 / 119 / 120**。
    pub merge_same_mode: i8,
    /// `getMergeNullMode()`（模型层 `NormalCell._$1`）。
    pub merge_null_mode: i8,
    /// 左 head 引用。阶段 B 逐格填入，初始 [`CellId::NONE`]。
    pub left_head: CellId,
    /// 上 head 引用。
    pub top_head: CellId,
    /// 模板声明的 leftHead 字符串（`ExtNormalCell.getLeftHead()`，分支 A）。
    /// ⚠ 与 [`Self::left_head`] 是**两个独立存储**，混用必错（见 trait 文档）。
    pub declared_left: Option<String>,
    /// 模板声明的 topHead 字符串。
    pub declared_top: Option<String>,
    /// 是否 `ExtExtendCell`（运行时类型）：18/19 的格为真，`cell00` 恒真。
    /// 归一化（`normalize_heads`）靠它判断「是否已到达最近的扩展格」。
    pub is_extender: bool,
    /// 计算态的值。字面量格在阶段 B 写入（`prepareCalculate` 的
    /// `setValue(srcPrepare.getValue())`，`ExtNormalCell.java:542`）；
    /// 带表达式的格留 [`Value::Null`]，由求值阶段填。
    pub value: Value,
    /// 显示值**声明项**。对应夹具 `disp_value` 键（`CalcJson.java:71-73`：
    /// 仅在**不等于** `String.valueOf(value)` 时才落盘，故是独立覆盖项而非格式化结果）。
    ///
    /// ⚠ 这是 `engine.srcPrepare.getDispValue()`（模板声明）那一支；
    /// `propSetMap[41]` 那一支在 [`Self::disp_override`]。取值请用 [`Self::disp_value`]。
    pub disp_declared: Option<String>,
    /// 显示值**覆盖项**（Java `propSetMap[41]`）。`setDispValue` 只在
    /// **与模板声明不同**时才写入（`ExtNormalCell.java:1653-1662`）。
    ///
    /// ⛔ `ExtNormalCell.clone():135` 把克隆的 `propSetMap` 置空 ⇒ 克隆**不继承覆盖**，
    /// 只留模板声明。这条由 [`crate::ext_algo::HeadState::clone_cell`] 承担。
    pub disp_override: Option<String>,
    /// 运行态（对应 Java 每格自带的 `NormalCellEngine`）。见 [`super::runtime`]。
    pub rt: CellRuntime,
    /// 左子格列表（Java `ExtExtendCell._$6`）：`left_head` 指向本格的格。
    /// ⛔ **仅扩展格与 root 维护**——`addCellLeftHead` 基类空实现、只有
    /// `ExtExtendCell.java:100-107` 有实体；普通 head 不登记。
    pub(crate) left_subs: Vec<CellId>,
    /// 上子格列表（`_$5`）。**扩列算法的施力点**（设计文档 §6.3，探针实证）。
    pub(crate) top_subs: Vec<CellId>,
    /// 扩展实例数组（`_$7`）。`sub_cells` 语义 = `len`，`None` 视作 1
    /// （`ExtExtendCell.extCellSize():126-128`；设计文档 §7）。
    ///
    /// ⛔ **必须是共享分配**：Java 的 `_$3` 是数组引用，锚点与全部克隆指向同一对象；
    /// 这里用 `Arc<[CellId]>` 对等实现。若改回「每格一份 `Vec`」，就是
    /// n 个克隆各持长度 n 的数组 ⇒ n²×4 字节（50k 明细 ≈ 9.9 GB，
    /// `scripts/bench_detail_mem.py` 实测）。
    pub(crate) ext_cells: Option<Arc<[CellId]>>,
}

impl HeadCell {
    /// 最小构造：跨度 1、无 head、无声明、无子格、按 extend 推 `is_extender`；
    /// **引擎出处默认等于当前坐标**（`HeadCell::new` 的调用点都在阶段 A 的建格序上，
    /// 那时计算态坐标 == 模板坐标，`ExtNormalCell.java:127-128` 也是这么写的）。
    /// 克隆由 [`crate::ext_algo`] 的字段浅拷贝完成 ⇒ 出处**自动被继承**，不要另行改写。
    /// `cell00` 或特殊格请直接改字段。
    pub fn new(row: i32, col: i32, extend: i8, row_type: i8) -> Self {
        HeadCell {
            row,
            col,
            src_row: row,
            src_col: col,
            extend,
            row_type,
            row_merge: 1,
            col_merge: 1,
            engine_row_merge: 1,
            engine_col_merge: 1,
            col_type: rpx_core::props::def::COL_TYPE,
            merge_same: rpx_core::props::def::MERGE_SAME,
            merge_null: rpx_core::props::def::MERGE_NULL,
            merge_same_mode: rpx_core::props::def::MERGE_SAME_MODE,
            merge_null_mode: rpx_core::props::def::MERGE_NULL_MODE,
            left_head: CellId::NONE,
            top_head: CellId::NONE,
            declared_left: None,
            declared_top: None,
            is_extender: matches!(extend, 18 | 19),
            value: Value::Null,
            disp_declared: None,
            disp_override: None,
            rt: CellRuntime::new(crate::literal::NORMAL_CELL_TYPE),
            left_subs: Vec::new(),
            top_subs: Vec::new(),
            ext_cells: None,
        }
    }

    /// `getDispValue()`（`ExtNormalCell.java:1644-1651`）：`propSetMap[41]` 优先，
    /// 否则回退模板声明。
    ///
    /// ⛔ 绝不按 `String.valueOf(value)` 合成默认值 —— 反例 `engine_disp` 的 `(2,3)`：
    /// `value == null` 而 `disp_value == "仅显示值"`。
    pub fn disp_value(&self) -> Option<String> {
        self.disp_override
            .clone()
            .or_else(|| self.disp_declared.clone())
    }

    /// `setDispValue(v)`（`ExtNormalCell.java:1653-1662`）：**只在 v 与模板声明不同**时
    /// 才写覆盖项（`v == null && 声明 == null` 也不写）。
    pub fn set_disp_value(&mut self, v: Option<String>) {
        let declared = self.disp_declared.clone();
        if v == declared {
            return;
        }
        self.disp_override = v;
    }

    /// `ExtExtendCell._$7`（扩展实例数组）的只读视图。非扩展格恒为 `None`
    /// （方法只存在于 `ExtExtendCell`）。
    pub(crate) fn ext_cells_ref(&self) -> Option<&[CellId]> {
        self.ext_cells.as_deref()
    }

    /// 共享分配的**廉价**克隆（`Arc::clone`，O(1)）。
    ///
    /// ⛔ 调用方不得把它 `to_vec()` 展开 —— 展开即把「共享」重新放大成
    /// 每格 O(n) 的拷贝，n 次读取就退回 O(n²)。
    pub(crate) fn ext_cells_arc(&self) -> Option<Arc<[CellId]>> {
        self.ext_cells.clone()
    }
}
