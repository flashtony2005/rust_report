//! 脚本化假网格（自 `tests.rs` 拆出，门禁行数）。
//!
//! 只做**事件记录**（求值顺序 / reset / 失效队列 / 引擎轮次）；`calculate()`
//! 按一次性脚本返回，脚本耗尽后默认完成（等价产品里的 FINISHED）。
//! `grow` 开关用来模拟「求值时发生扩展，把 `_$16`/`_$15` 缓存改大」（见 §12.1 的 U9），
//! 这是通道 4/5 循环边界那条约束的钉子所必需的。

use std::collections::{HashMap, VecDeque};

use super::super::sched::SchedGrid;
use super::super::{state, Flow, Suspend};
use crate::arena::CellId;
use crate::error::EngineError;

/// `calculate()` 的一次返回：完成，或挂起到 (目标格, 状态)。
pub(crate) enum Step {
    Ok,
    To(CellId, u8),
}

/// 假格：坐标 / 生效 extensible / 是否引擎矩阵本体 / 是否恒抛 state 6 /
/// 是否恒抛终局错误 / 求值时是否"插入一行一列"。
pub(crate) struct FakeCell {
    pub(crate) row: i32,
    pub(crate) col: i32,
    pub(crate) ext: i8,
    pub(crate) orig: bool,
    pub(crate) loop6: bool,
    pub(crate) fatal: bool,
    /// 模拟 `extendRow`/`extendCol` 把 `_$16`/`_$15` 缓存改大（U9）。
    pub(crate) grow: bool,
}

/// 脚本化假网格：`SchedGrid` 的事件记录实现。
pub(crate) struct FakeGrid {
    pub(crate) rows: i32,
    pub(crate) cols: i32,
    pub(crate) cells: Vec<FakeCell>,
    pub(crate) at: HashMap<(i32, i32), CellId>,
    pub(crate) script: HashMap<CellId, VecDeque<Step>>,
    /// calculate 的调用顺序（含 chase 换格后的重复调用）。
    pub(crate) prep: Vec<CellId>,
    pub(crate) resets: Vec<CellId>,
    pub(crate) cal8: Vec<CellId>,
    pub(crate) failq: Vec<CellId>,
    pub(crate) engine: Vec<u8>,
    pub(crate) interrupted: bool,
}

impl FakeGrid {
    pub(crate) fn new(rows: i32, cols: i32) -> Self {
        FakeGrid {
            rows,
            cols,
            cells: Vec::new(),
            at: HashMap::new(),
            script: HashMap::new(),
            prep: Vec::new(),
            resets: Vec::new(),
            cal8: Vec::new(),
            failq: Vec::new(),
            engine: Vec::new(),
            interrupted: false,
        }
    }
    /// 创建格但**不落槽位**（不会被扫描到，供脚本引用）。
    pub(crate) fn cell(&mut self, row: i32, col: i32, ext: i8, orig: bool) -> CellId {
        let id = CellId(self.cells.len() as u32);
        self.cells.push(FakeCell {
            row,
            col,
            ext,
            orig,
            loop6: false,
            fatal: false,
            grow: false,
        });
        id
    }
    /// 创建格并坐在 (row, col) 槽位上（会被扫描到）。
    pub(crate) fn put(&mut self, row: i32, col: i32, ext: i8, orig: bool) -> CellId {
        let id = self.cell(row, col, ext, orig);
        self.at.insert((row, col), id);
        id
    }
    pub(crate) fn script(&mut self, id: CellId, steps: Vec<Step>) {
        self.script.insert(id, steps.into());
    }
}

impl SchedGrid for FakeGrid {
    fn dims(&self) -> (i32, i32) {
        (self.rows, self.cols)
    }
    fn get_calc(&self, r: i32, c: i32) -> CellId {
        *self.at.get(&(r, c)).unwrap_or(&CellId::NONE)
    }
    fn row(&self, id: CellId) -> i32 {
        self.cells[id.0 as usize].row
    }
    fn col(&self, id: CellId) -> i32 {
        self.cells[id.0 as usize].col
    }
    fn extensible(&self, id: CellId) -> i8 {
        self.cells[id.0 as usize].ext
    }
    fn is_original(&self, id: CellId) -> bool {
        self.cells[id.0 as usize].orig
    }
    fn calculate(&mut self, id: CellId) -> Result<(), Flow> {
        self.prep.push(id);
        if self.cells[id.0 as usize].loop6 {
            return Err(Flow::Suspend(Suspend::new(id, state::SPECIAL))); // 恒抛 6：活锁用例
        }
        if self.cells[id.0 as usize].fatal {
            return Err(Flow::Fatal(EngineError::UnknownCell {
                id: format!("fatal#{}", id.0),
            }));
        }
        // 模拟扩展：当场把两个尺寸缓存各改大 1（`_$16/_$15` 的 `+=`，U9）。
        if self.cells[id.0 as usize].grow {
            self.rows += 1;
            self.cols += 1;
        }
        match self.script.entry(id).or_default().pop_front() {
            None | Some(Step::Ok) => Ok(()),
            Some(Step::To(c, s)) => Err(Flow::Suspend(Suspend::new(c, s))),
        }
    }
    fn prepare_eval(&mut self, _id: CellId) {}
    fn interrupted(&self) -> bool {
        self.interrupted
    }
    fn reset(&mut self, id: CellId) {
        self.resets.push(id);
    }
    fn set_cal_state(&mut self, id: CellId, st: u8) {
        if st == state::THIRD {
            self.cal8.push(id);
        }
    }
    fn fail_queued(&self, id: CellId) -> bool {
        self.failq.contains(&id)
    }
    fn fail_queue_push(&mut self, id: CellId) {
        self.failq.push(id);
    }
    fn set_engine_state(&mut self, st: u8) {
        self.engine.push(st);
    }
}
