//! [`CalcCtx`]：顶层 `calculate()` 的**唯一可变状态持有者**。
//!
//! 算法正文见 `docs/m2-engine-design.md` **§5.4**（本轮的接线设计）。Java 侧的对应物
//! 是 `ExtCellSet` 自身（它同时是"计算矩阵 + 索引 + 调度器"）加上每格自带的
//! `NormalCellEngine`（后者落在 [`crate::heads::CellRuntime`] 里）。
//!
//! # 为什么是"一个 struct 实现三个 trait"
//!
//! 求值链上三处需要回看同一个世界：
//!
//! | trait | 谁在调 | 需要什么 |
//! |-------|--------|----------|
//! | [`SchedGrid`] | [`crate::eval::sched`] 的通道 4/5/6 | 矩阵尺寸、槽位、`extensible`、`calculate`、`prepare_eval` |
//! | [`EvalCtx`] | [`crate::expr::evaluate`] 与 [`crate::expr::resolve`] | head 图、两指针、格值、数据集 |
//! | [`DsCurrentHost`] | [`crate::calc::ds_current`] 的三件套 | DS current 三件套、`_$4`/`_$7` |
//!
//! 三者的状态**互相改写**（扩展改矩阵尺寸 → 通道循环边界随之变；求值改 `current`
//! 并借此改 DS 指针 → 下一次 head 查找读的又是它）。拆成多个对象就必须引入
//! 同步点，而同步点漏一处就是静默错值。Java 的做法也是一个大对象 + 每格一个
//! 小 engine，这里照做。
//!
//! # 谁是谁（⛔ 三个概念别混）
//!
//! - **`current`（`_$21`）**：当前正在求值的格。相对引用的基准、`@` 的取值对象。
//!   由通道在每格前写（[`SchedGrid::prepare_eval`]），也被 `sum(A:B)` 临时改写。
//! - **`saved`（`_$20`）**：`saveCurrent()` 的快照。**只有** `$B2`（isSaved）与
//!   `sum(A:B)` 的 `resumeCurrent()` 用它。
//! - **`spots[i]`（每个 DataSet 的 `current` 三件套）**：`ds.current` 指针。
//!   由逐格 `calculate` 内部按 head 链重算（见 [`crate::calc::ds_current`] 的模块文档）。
//!   `current` 换了**不等于** DS 指针换了，反之亦然。
//!
//! # 镜像：`spots` 是权威，`DataSet.current` 是投影
//!
//! [`EvalCtx::ds`] 交给求值器的是 [`crate::expr::DataSet`]，它只有一个
//! `current: DsCur`（`Root | Row(i)`）。而 [`DsCurrentHost`] 要的是三件套
//! （`value` / `is_left_top` / `current_row`）。⇒ 以 `spots` 为权威，
//! 每次 `set_current` 顺手把它投影进 `DataSet.current`。
//!
//! ⛔ **`spots[i].value == None` 投影成 `Root` 是安全的**：`reset_ds_current` 造的
//! "全 None" 状态**没有任何求值窗口停在它上面** —— 三处调用点
//! （`ExtCellSet.java:1497-1499` / `:1501-1503` / `:1522-1524`）都是
//! `resetDSCurrent` 紧跟 `setDSCurrent`，后者会把全部 None 填成 root
//! （`ds_current::set_ds_current`）。而逐格 `calculate` 内部的
//! `setDSCurrent`（`ExtNormalCell.java:602-613`）同样是"reset → left 链 → top 链 →
//! 只填空"四段连做。⇒ 投影成 `Root` 与 Java 在 `null` 上取 `getCurrentData`
//! 的行为一致（产品那条路会抛，本项目不会走到）。

use crate::arena::CellId;
use crate::calc::ds_current::{DsCurrentHost, DsInstance, DsSpot, HeadPairTracker, ROOT_ROW};
use crate::error::Result;
use crate::expr::ds::{DataSet, DsCur};
use crate::heads::HeadState;
use crate::value::Value;

mod eval_ctx;
mod sched_grid;

/// 顶层求值的全部可变状态。见模块文档。
#[derive(Debug)]
pub struct CalcCtx {
    /// head 图工作台（也承载每格的运行态 [`crate::heads::CellRuntime`]）。
    ///
    /// ⛔ **扩展会往 `grid.cells` 追加格、往 `by_pos` 插槽位、并改
    /// `grid.row_size`/`col_size`** —— 通道 4/5 的循环边界必须因此每轮重读
    /// （见 [`SchedGrid::dims`] 的文档）。
    pub grid: HeadState,
    /// 运行时数据集，下标即 `DsInstance.ds`。
    pub ds: Vec<DataSet>,
    /// `_$21`：当前求值格。
    current: CellId,
    /// `_$20`：`saveCurrent()` 的快照。
    saved: CellId,
    /// `_$14`：引擎级状态（0/2/3/4/5/6/7/8）。
    engine_state: u8,
    /// `_$19`：`(leftHead, topHead)` 对变化跟踪器。
    pub(crate) tracker: HeadPairTracker,
    /// 每个数据集的 current 三件套（**权威**，见模块文档）。
    spots: Vec<DsSpot>,
    /// `_$24`：state 8 级联失效队列。
    fail_queue: Vec<CellId>,
    /// DS 旁路：本次求值的"最终结果是一行"记录（`(ds 下标, 行号)`）。
    note_row: Option<(usize, usize)>,
    /// DS 旁路：本次求值的"最终结果是全量行"记录。
    note_list: Option<(usize, Vec<usize>)>,
}

impl CalcCtx {
    /// 用阶段 B 的产物 + 数据集建一个上下文。
    ///
    /// `_$14` 初值按 Java 的字段初值取 **0**（`ExtCellSet.java:108`）：入口守卫
    /// `_$14 <= 4` 因此放行第一次调用。⛔ `stage_b` 的产物**不能**拿来跑第二遍
    /// （`prepareCalculate` 已经把 `extensible` 改写过了），所以这里不做任何
    /// 复用检查 —— 由 [`super::run::calculate`] 的文档声明"一张网格只跑一次"。
    pub fn new(grid: HeadState, ds: Vec<DataSet>) -> Self {
        let n = ds.len();
        CalcCtx {
            grid,
            ds,
            current: CellId::NONE,
            saved: CellId::NONE,
            engine_state: 0,
            tracker: HeadPairTracker::new(),
            spots: vec![DsSpot::empty(); n],
            fail_queue: Vec::new(),
            note_row: None,
            note_list: None,
        }
    }

    /// `_$14`。
    pub fn get_state(&self) -> u8 {
        self.engine_state
    }

    /// 清空 `_$24`（失效队列）—— 顶层编排每轮新建（`ExtCellSet.java:1492`）。
    pub(crate) fn clear_fail_queue(&mut self) {
        self.fail_queue.clear();
    }

    /// `setState(byte)`（`ExtCellSet.java:1759-1761`）。
    pub fn set_state(&mut self, st: u8) {
        self.engine_state = st;
    }

    /// 清空 DS 旁路（每次逐格求值**开始前**调，见 [`EvalCtx::clear_ds_notes`]）。
    pub(crate) fn clear_notes(&mut self) {
        self.note_row = None;
        self.note_list = None;
    }

    /// 把 `spots[i]` 投影进 `ds[i].current`（见模块文档的"镜像"一节）。
    fn mirror(&mut self, i: usize) {
        let cur = match self.spots[i].value {
            Some(v) if !v.is_root() => DsCur::Row(v.row),
            _ => DsCur::Root,
        };
        if let Some(set) = self.ds.get_mut(i) {
            set.set_current(cur);
        }
    }

    /// 一个格的运行态（越界 → `None`）。
    pub(crate) fn rt(&self, c: CellId) -> Option<&crate::heads::CellRuntime> {
        self.grid.get(c).map(|h| &h.rt)
    }

    /// 一个格的运行态（可变）。
    pub(crate) fn rt_mut(&mut self, c: CellId) -> Option<&mut crate::heads::CellRuntime> {
        let i = c.index()?;
        self.grid.cells.get_mut(i).map(|h| &mut h.rt)
    }

    /// `cell.getValue()`（`ExtNormalCell.java:1634-1636`）：读 `cellValue`。
    pub(crate) fn value_of(&self, c: CellId) -> Value {
        self.grid
            .get(c)
            .map(|h| h.value.clone())
            .unwrap_or(Value::Null)
    }

    /// `cell.setValue(v)`（`ExtNormalCell.java:1639-1642`）：写值 **并置 bit30**。
    ///
    /// ⛔ 两件事绑在一起是本函数存在的唯一理由 —— Java 的 `setCalc(true)` 就藏在
    /// `setValue` 里，"写值"与"标记已求值"在产品里**不可分**。分开写迟早漏一处，
    /// 后果是同一格被通道 4/5 再算一遍。
    pub(crate) fn set_value(&mut self, c: CellId, v: Value) {
        if let Some(h) = self.grid.cells.get_mut(c.0 as usize) {
            h.value = v;
            h.rt.set_calc_true();
        }
    }

    /// `cell.setCalc(calc)`（`ExtNormalCell.java:291-298`）：`calc=false` 时**连值一起清掉**。
    pub(crate) fn set_calc(&mut self, c: CellId, calc: bool) {
        if let Some(h) = self.grid.cells.get_mut(c.0 as usize) {
            if calc {
                h.rt.set_calc_true();
            } else {
                h.rt.set_calc_false();
                h.value = Value::Null;
            }
        }
    }

    /// 裸写字段 `cellValue`，**不置 bit30** —— 对应 Java 里对字段的直接赋值
    /// （`this.cellValue = oldVal`，`ExtNormalCell.java:671` / `ExtExtendCell.java:428/:461`）。
    ///
    /// ⛔ 只用于 `calcDispValue` 失败后的回滚（catch 块里"先写旧值、再 `setCalc(calc)`"
    /// 两步连做 —— `calc=false` 时后者会把值再清成 null，与产品逐行同构）。
    /// 求值路径必须用 [`Self::set_value`]（写值 + 置位是产品 `setValue` 的绑定语义）。
    pub(crate) fn write_value_raw(&mut self, c: CellId, v: Value) {
        if let Some(h) = self.grid.cells.get_mut(c.0 as usize) {
            h.value = v;
        }
    }

    /// `cell.reset()`（`ExtNormalCell.java:322-324`）：`rowMerge &= 0x3FFFFFFF`。
    ///
    /// ⛔ **它只清两个计算标志，不清值** —— 与
    /// [`Self::set_calc`]`(c, false)` 不是一回事（后者清值）。两条都有调用点
    /// （`reset` 在通道 5/6 的失效路径，`setCalc(false)` 在 `prepareCalculate`）。
    pub(crate) fn reset_flags(&mut self, c: CellId) {
        if let Some(h) = self.grid.cells.get_mut(c.0 as usize) {
            h.rt.calculated = false;
            h.rt.demanded = false;
        }
    }

    /// `cell.getCellType()`。
    pub(crate) fn cell_type(&self, c: CellId) -> i8 {
        self.grid
            .get(c)
            .map(|h| h.rt.cell_type)
            .unwrap_or(crate::literal::NORMAL_CELL_TYPE)
    }

    /// `getDispValue()`（`ExtNormalCell.java:1644-1651`）。求值链内暂无读者
    /// （disp 消费点在导出侧直读 `HeadCell`）；保留作 `set_disp_value` 的对称口。
    #[allow(dead_code)]
    pub(crate) fn disp_value(&self, c: CellId) -> Option<String> {
        self.grid.get(c).and_then(|h| h.disp_value())
    }

    /// `setDispValue(v)`（`ExtNormalCell.java:1653-1662`）。
    pub(crate) fn set_disp_value(&mut self, c: CellId, v: Option<String>) {
        if let Some(h) = self.grid.cells.get_mut(c.0 as usize) {
            h.set_disp_value(v);
        }
    }
}

impl DsCurrentHost for CalcCtx {
    fn cell00(&self) -> CellId {
        CellId::ROOT
    }

    fn left_head(&self, c: CellId) -> CellId {
        self.grid
            .get(c)
            .map(|h| h.left_head)
            .unwrap_or(CellId::NONE)
    }

    fn top_head(&self, c: CellId) -> CellId {
        self.grid.get(c).map(|h| h.top_head).unwrap_or(CellId::NONE)
    }

    fn is_extender(&self, c: CellId) -> bool {
        self.grid.get(c).is_some_and(|h| h.is_extender)
    }

    fn instance(&self, c: CellId) -> Option<DsInstance> {
        self.rt(c).and_then(|r| r.instance)
    }

    fn ext_cells(&self, c: CellId) -> Vec<CellId> {
        self.grid
            .get(c)
            .and_then(|h| h.ext_cells_ref())
            .cloned()
            .unwrap_or_default()
    }

    fn ds_count(&self) -> usize {
        self.ds.len()
    }

    fn ds_spot(&self, ds: usize) -> DsSpot {
        self.spots.get(ds).copied().unwrap_or_else(DsSpot::empty)
    }

    fn ds_set_current(&mut self, ds: usize, v: Option<DsInstance>, is_left_top: bool) {
        if let Some(s) = self.spots.get_mut(ds) {
            s.set(v, is_left_top);
        }
        self.mirror(ds);
    }

    fn ds_root(&self, ds: usize) -> DsInstance {
        DsInstance { ds, row: ROOT_ROW }
    }

    fn ds_null_row(&self, ds: usize) -> DsInstance {
        DsInstance::new(ds, crate::calc::ds_current::NULL_ROW)
    }

    fn ds_row_cross(&self, ds: usize, recv: DsInstance, other: DsInstance) -> Result<DsInstance> {
        // ⛔ 只移植 `Row.cross`（`Row.java:68-79`）。`Group.cross`（阈值化集合运算）
        // **未移植** —— 12 个夹具里 `cross` 的接收者恒是 Row：接收者非 null 说明它被
        // `setDSCurrentLeftHead` 写过，而那里写的 `_$4` 恒是 `DsValue` 行实例
        // （`ExtExtendCell.java:251-258`），根 Group 是由 `setDSCurrent`（"只填空"、
        // `:572-574`）写的、`is_left_top` 为 false，进不了本函数。
        crate::calc::ds_current::row_cross_default(self, ds, recv, other)
    }

    fn tracker_set_changed(&mut self, changed: bool) {
        self.tracker.set_changed(changed);
    }
}

#[cfg(test)]
mod tests;
