//! 单元格的**运行态**（`HeadCell::rt`）—— 对应 Java 每个格对象自带的 `NormalCellEngine`。
//!
//! # 为什么放在格对象里，而不是另开一张平行数组
//!
//! Java 的 `ExtNormalCell` 字段里有一个 `protected NormalCellEngine engine`（`:70`），
//! 承载 `expression` / `state` / `isDSFunction` / `srcCurrent` / `extensible` 等。
//! `clone()` 是 `Object.clone()` 的浅拷贝 ⇒ **`engine` 引用被克隆共享**
//! （`ExtNormalCell.java:133-148` 只改 `propSetMap` 与 `rowMerge`，没动 `engine`）。
//!
//! 本 crate 曾在别处用"与 `HeadState.cells` 平行的 `Vec`"建模运行态 —— 那是**会静默错位**的：
//! 扩展算法（[`crate::ext_algo`]）在 `extend_row`/`extend_col` 里调 [`crate::ext_algo::HeadState::clone_cell`]
//! 往 `cells` **追加**格，平行数组无从得知，于是此后新格的下标全部错一格。
//! 把运行态放进格对象后，"克隆自动带走运行态"与产品同构，也不再需要同步点。
//!
//! # ⚠ 与 Java 的一处**有意偏离**（已声明，当前无夹具可辨）
//!
//! Java 的克隆**共享** `engine` 对象 ⇒ 写 `engine.state`（`setCalState`）会同时影响锚点与
//! 全部克隆；本 crate 的 `clone_cell` 是**逐字段拷贝** ⇒ `rt` 各格独立。
//!
//! 唯一的可观察差异在 `ExtNormalCell.calculate:640` 的第二项：
//! `currentState == 8 && engine.state == 8`（state 8 级联失效路径）。
//! 而 `engine.state` 只在 `setCalState(8)` 时被写成 8，那条路径要求
//! `_$14 == 7 && (_$12 & 4) != 0`（`ExtCellSet.java:1702`），`_$12 |= 4` 只在
//! `reflush():2265` 置位 ⇒ **普通单次 calc 不进 8**。
//! 再加上 `Expr::get_state()` 在本 crate 的可达值域只有 `{4}`
//! （`crates/rpx-engine/tests/expr_state_tripwire.rs` 用 40 条夹具公式钉住），
//! 于是 state 8 分支在 12 个夹具上**恒不可达** ⇒ 本偏离无夹具级信号。
//! 钉子：`super::runtime_tests::cloning_copies_runtime_state_instead_of_sharing_it`。

use crate::calc::ds_current::DsInstance;
use crate::expr::Expr;

/// 一个格的运行态。
///
/// 字段与 Java `NormalCellEngine` 的对应：
///
/// | Rust | Java | 语义 |
/// |------|------|------|
/// | [`Self::expr`] | `engine.expression: Object`（`ExtNormalCell.java:104`） | 键 40 的表达式；`None` = 字面量格 |
/// | [`Self::cell_type`] | `getCellType()`（`engine.srcPrepare.getCellType()`） | 模板类型；`-64` 普通 / `-58` HTML / `-56` 富文本 / `-60` 子报表 / `-59` 空 / `-61` 图形 / `-57` 条形码 |
/// | [`Self::calculated`] | `rowMerge` 的 **bit30**（`setCalc(true)` 置位，`ExtNormalCell.java:291-298`） | 「已求值」。⛔ **由写值函数置位**，不是由 `calculate` 显式置 —— 见 [`super::super::calc::cell_eval`] |
/// | [`Self::demanded`] | `rowMerge` 的 **bit31**（`Integer.MIN_VALUE`，`testValue():305`） | 「已被别人要求求值过一次」。第二次再被要求 ⇒ 判**循环引用**（`:307` 抛 state 1） |
/// | [`Self::instance`] | `ExtExtendCell._$4: DsValue`（`:37`） | 本格"当前代表的那个 DsValue 实例"；`None` = Java 的 `null` |
///
/// # ⛔ bit30/bit31 为什么可以是两个独立 `bool`
///
/// Java 把两位打包在 `rowMerge` 里，但：
/// - `getRowMerge()` 是 `rowMerge & 0x3FFFFFFF`（`ExtNormalCell.java:236`），而夹具的
///   `source.row_merge` 读的正是 `getRowMerge()`（`CalcJson.java:97`）
///   ⇒ **两位都不落盘**，不存在"序列化形态要一致"的约束；
/// - 两位之间**没有进位/耦合**：`setCalc(true)` 只置 bit30；`testValue` 只置 bit31；
///   `nonCalc()` 只读 bit30；`testValue` 的 `rowMerge > 0` 是**裸值比较**，被判成"负"的唯一
///   原因就是 bit31 已置（逻辑跨度恒为正）⇒ 等价于"看 `demanded`"。
///
/// 唯一被 `& Integer.MAX_VALUE` 抹掉的是 bit31（`ExtNormalCell.clone:136`），
/// 而它抹 bit31 的**目的**恰好就是「克隆不继承『已被要求求值』」⇒ 本模型里
/// [`crate::ext_algo::HeadState::clone_cell`] 显式把 `demanded` 置回 `false`，逐条对应。
///
/// ⚠ 但 `ExtNormalCell.clone:136` 的 `&= Integer.MAX_VALUE` **保留 bit30**
/// ⇒ 克隆**继承「已求值」**。这正是「扩展出来的格不会被通道 4/5 再算一遍」的机制本体；
/// `clone_cell` 因此**不**重置 `calculated`。这条是值关键的，别"顺手清干净"。
#[derive(Debug, Clone, Default)]
pub struct CellRuntime {
    /// 键 40 的表达式 AST；`None` = 字面量格（`ExtNormalCell.java:539` 的 `expression == null`）。
    /// 键 40 的表达式 AST；`None` = 字面量格（`ExtNormalCell.java:539` 的 `expression == null`）。
    ///
    /// ⛔ **共享分配**：Java 的克隆共享 `engine` 引用（`ExtNormalCell.clone:133-148`
    /// 只改 `propSetMap`/`rowMerge`，没动 `engine`），本字段用 `Arc` 对等实现。
    /// 逐格深拷贝 AST 会让 n 个扩展克隆各持一份表达式树 —— 50k 明细实测多出
    /// 约 1.2 GB（`scripts/bench_detail_mem.py`）。
    pub expr: Option<std::sync::Arc<Expr>>,
    /// 模板单元格类型（`INormalCell.getCellType()`）。
    ///
    /// ⛔ 兜底取 `-64`（普通）：`-64` 是唯一"什么特殊都不做"的类型，
    /// 且 `ExtNormalCell.calculate:663` / `ExtExtendCell.calculate:388` 的
    /// `calcDispValue` 触发集都含它。取 `0` 之类的假值会让那些分支静默失效。
    pub cell_type: i8,
    /// 已求值（rowMerge bit30）。
    pub calculated: bool,
    /// 已被要求求值过（rowMerge bit31）。第二次要求 ⇒ 循环引用。
    pub demanded: bool,
    /// `engine.state`（`NormalCellEngine.java:33` 的 `protected byte state`）。
    ///
    /// ⛔ **缺省 0，不是 4**：Java 只在 `checkExtend:508`（有表达式的那条）与
    /// `prepareCalculate:533`（`-61/-57/-60` 三种类型那条）写它，字面量格**从不写**
    /// ⇒ 字面量格的 `engine.state` 是字段初值 `0`。
    /// 这个差别有判别力：`ExtNormalCell.calculate:637` 是
    /// `engine.state > cs.getState()`（严格大于）才挂起 —— 取 4 会让"字面量格"这条路
    /// 的守卫数值不同。虽然 `calculate` 对无表达式的格会提前返回（`expression == null`），
    /// 但 `testValue`/`setCalState` 也都读它，别乱填。
    pub state: u8,
    /// `_$4`：本格当前代表的 DsValue 实例（只有扩展格会写）。
    pub instance: Option<DsInstance>,
    /// 非值表达式的属性表达式（Java `propEngMap` / RowCellEngine / ColCellEngine）。
    /// Clone 时随格一起复制；扩展实例按各自 current 求值。
    pub property_exprs: Vec<(u8, Expr)>,
    /// `calcProperties` 的运行态覆盖值（Java `propSetMap` / `customPropSetMap`）。
    pub property_values: Vec<(u8, crate::value::Value)>,
}

impl CellRuntime {
    /// 一个"还没算过、什么都没有"的运行态。`cell_type` 缺省 `-64`（普通格）、
    /// `state` 缺省 `0`（见 [`Self::state`] 的说明）。
    pub fn new(cell_type: i8) -> Self {
        CellRuntime {
            expr: None,
            cell_type,
            calculated: false,
            demanded: false,
            state: 0,
            instance: None,
            property_exprs: Vec::new(),
            property_values: Vec::new(),
        }
    }

    /// `nonCalc()`（`ExtNormalCell.java:287-289`）：`(rowMerge >> 30 & 1) == 0`。
    ///
    /// 语义 = 「本格**还没有**求值过」。⛔ 名字是反的直觉：`nonCalc()` 为真才轮到 `calculate` 干活。
    pub fn non_calc(&self) -> bool {
        !self.calculated
    }

    /// `setCalc(true)`（`:291-293`）：置 bit30。**不碰值**。
    pub fn set_calc_true(&mut self) {
        self.calculated = true;
    }

    /// `setCalc(false)`（`:294-297`）：清 bit30 **并且把值清空**。
    ///
    /// ⛔ 清值是该方法的**副作用**而不是调用方的另一步 —— 忘掉它会让
    /// "回滚到进入前的状态"变成"值还在但被当成没算过"，于是下一趟通道 4/5
    /// 会拿旧值当新值。Rust 侧由 [`crate::calc::cell_eval`] 的 `set_calc` 统一承担。
    pub fn set_calc_false(&mut self) {
        self.calculated = false;
    }
}
