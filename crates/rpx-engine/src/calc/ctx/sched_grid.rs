//! `impl SchedGrid for CalcCtx` —— 三条调度通道（[`crate::eval::sched`]）眼中的世界。
//!
//! 语义全部在 [`SchedGrid`] 与 [`crate::eval::sched`] 的文档里；这里只有两处
//! 需要解释，其余是薄转发。
//!
//! # `prepare_eval`（每格求值前的钩子，`ExtCellSet.java:1306-1309` 等 8 处同构）
//!
//! Java 在通道循环里每次 `calculate` 前连做两件事：
//!
//! ```java
//! this._$48 = cell;                          // 引擎的"当前格"指针
//! this._$46.setCell(cell, this._$37);        // (leftHead,topHead) 对变化跟踪
//! ```
//!
//! 对应到本 crate：`_$48` 就是 [`CalcCtx::current`]；`_$46.setCell` 按
//! `lIlIlllIIllllIII.java:21-39` 的语义读 `cell.getCellLeftHead()/getCellTopHead()`
//! 后调 [`HeadPairTracker::set_cell`]（reportlite 版已逐行核对：
//! `changed` 为真时只更新记录；为假时 `(left==cell00 && top==cell00) || left 变`
//! 置真，否则 top 变置真，否则显式置假 —— 与 `types.rs` 的实现逐分支等价）。
//! `cell00`（`_$37`）恒是 [`CellId::ROOT`]。
//!
//! # `calculate` 的落点
//!
//! 委托给 [`crate::calc::cell_eval::cell_calculate`] —— 逐格 `calculate` 的
//! 移植（`ExtNormalCell.calculate:633-676` + `ExtExtendCell.calculate:355-483`）。
//! `Ok(())` 完成 / `Err(Flow::Suspend)` 请求换格 / `Err(Flow::Fatal)` 终局，
//! 三态契约见 [`SchedGrid::calculate`] 的文档。
//!
//! # 与 Java 的两处已声明偏离
//!
//! 1. `interrupted()` 恒 `false`：Java 的 `_$8()`（`ExtCellSet.java:233-237`，
//!    reportlite 版通道循环内逐格调用的就是它）读 `_$11`（`interrupt()` 置位）。
//!    本 crate 是库形态，没有外部中断入口；真要支持时在 `CalcCtx` 加标志并接到
//!    这里，通道侧（`channel4/5/6` 的 `g.interrupted()` 分支）已就绪。
//! 2. `row`/`col` 对越界 id 返回哨兵而不是 panic：通道只在 `!cell.is_none()` 之后
//!    才读坐标（`slot_matches` 在判空之后调），越界值永远不会参与比较 ——
//!    用 `i32::MIN` 兜底只是防御，不是语义。

use crate::arena::CellId;
use crate::eval::Flow;
use crate::eval::sched::SchedGrid;

use super::CalcCtx;

impl SchedGrid for CalcCtx {
    fn dims(&self) -> (i32, i32) {
        // `_$16`/`_$15`（`ExtCellSet.java` 的计算矩阵尺寸缓存，含 0 号行/列）。
        // ⛔ 通道循环每轮重读本值：扩展会在 `calculate` 里改写它（`sched.rs` 文档）。
        (self.grid.row_size, self.grid.col_size)
    }

    fn get_calc(&self, row: i32, col: i32) -> CellId {
        // `_$54.get(i, j)`：计算矩阵取格；越界/空槽 ⇒ `NONE`。
        self.grid.by_pos.get(&(row, col)).copied().unwrap_or(CellId::NONE)
    }

    fn row(&self, id: CellId) -> i32 {
        self.grid.get(id).map(|h| h.row).unwrap_or(i32::MIN)
    }

    fn col(&self, id: CellId) -> i32 {
        self.grid.get(id).map(|h| h.col).unwrap_or(i32::MIN)
    }

    fn extensible(&self, id: CellId) -> i8 {
        // `cell.getExtensible()`：阶段 B `checkExtend`/`prepareCalculate` 改写后的生效值。
        self.grid.get(id).map(|h| h.extend).unwrap_or(16)
    }

    fn is_original(&self, id: CellId) -> bool {
        // `cell.getSource() == this`：引擎矩阵本体格。克隆的 (src_row, src_col)
        // 与锚点相同，而 `by_src` 只登记本体 ⇒ `source_of(clone) == 锚点 ≠ clone`。
        // `cell00` 的出处是它自己 ⇒ 本体 ✓。
        self.grid.source_of(id) == id
    }

    fn calculate(&mut self, id: CellId) -> Result<(), Flow> {
        crate::calc::cell_eval::cell_calculate(self, id)
    }

    fn prepare_eval(&mut self, id: CellId) {
        // `_$48 = cell; _$46.setCell(cell, cell00)`（模块文档）。
        self.current = id;
        let (left, top) = match self.grid.get(id) {
            Some(h) => (h.left_head, h.top_head),
            None => (CellId::NONE, CellId::NONE),
        };
        self.tracker.set_cell(left, top, CellId::ROOT);
    }

    fn interrupted(&self) -> bool {
        // 见模块文档"偏离 1"。
        false
    }

    fn reset(&mut self, id: CellId) {
        // `cell.reset()`（`ExtNormalCell.java:322-324`）＝ `rowMerge &= 0x3FFFFFFF`：
        // **只清两个计算标志、不清值**。⛔ 与 `setCalc(false)`（连值一起清）不是
        // 一回事 —— 通道 5 的 state 6/8 失效路径调的是 `reset`（`:1398`/`:1410`）。
        self.reset_flags(id);
    }

    fn set_cal_state(&mut self, id: CellId, st: u8) {
        // `setCalState(byte)`（`ExtNormalCell.java:2672-2674`）＝ `engine.state = state`。
        if let Some(rt) = self.rt_mut(id) {
            rt.state = st;
        }
    }

    fn fail_queued(&self, id: CellId) -> bool {
        // `_$24.indexOf(c) >= 0`（`:1412`）。
        self.fail_queue.contains(&id)
    }

    fn fail_queue_push(&mut self, id: CellId) {
        self.fail_queue.push(id);
    }

    fn set_engine_state(&mut self, st: u8) {
        // `_$14 = st`（通道切换：4/5/6/7，`sched.rs` 的 `set_engine_state` 调用点）。
        self.engine_state = st;
    }
}
