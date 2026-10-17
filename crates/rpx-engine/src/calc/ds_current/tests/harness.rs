//! `ds_current` 单测用的假宿主（自 `tests.rs` 拆出，门禁行数）。
//!
//! ⛔ 刻意**不做任何简化**：`_$4`（[`FakeCell::inst`]）与 `_$7`（[`FakeCell::ext`]）
//! 是两个独立字段 —— 这正是 `docs/m2-engine-design.md` §5.2.6 的产品探针实测结论
//! （`engine_sum` 的 `(2,2)` 是 `_$7 == null` 却有 per-instance `_$4` 的"跟随型"格）。
//! 谁把它们合并成一个数组，"跟随型"那组单测的**前提**就变了。

use super::super::types::{DsInstance, DsSpot, HeadPairTracker, NULL_ROW};
use super::super::{row_cross_default, DsCurrentHost};
use crate::arena::CellId;

/// 假格。
#[derive(Clone)]
pub(crate) struct FakeCell {
    pub(crate) left: CellId,
    pub(crate) top: CellId,
    pub(crate) extender: bool,
    /// `_$4`。
    pub(crate) inst: Option<DsInstance>,
    /// `_$7`。
    pub(crate) ext: Vec<CellId>,
}

impl FakeCell {
    /// 普通格（`ExtNormalCell`）：不是扩展格，无 `_$4`/`_$7`。
    pub(crate) fn plain() -> Self {
        FakeCell {
            left: CellId::NONE,
            top: CellId::NONE,
            extender: false,
            inst: None,
            ext: Vec::new(),
        }
    }

    /// 扩展格（`ExtExtendCell`）。
    pub(crate) fn extender(inst: Option<DsInstance>, ext: Vec<CellId>) -> Self {
        FakeCell {
            left: CellId::NONE,
            top: CellId::NONE,
            extender: true,
            inst,
            ext,
        }
    }
}

/// 假宿主：一张格表 + 若干 DataSet 的 `current` 三件套。
pub(crate) struct Fake {
    pub(crate) cells: Vec<FakeCell>,
    pub(crate) ds: Vec<DsSpot>,
    pub(crate) root: Vec<DsInstance>,
    pub(crate) null: Vec<DsInstance>,
    pub(crate) tracker: HeadPairTracker,
}

impl Fake {
    /// `n_ds` 个数据集；下标 0 号推入的格即 `cell00`。
    pub(crate) fn new(n_ds: usize) -> Self {
        Fake {
            cells: vec![FakeCell::plain()],
            ds: vec![DsSpot::empty(); n_ds],
            root: (0..n_ds).map(|d| DsInstance::new(d, 1000)).collect(),
            null: (0..n_ds).map(|d| DsInstance::new(d, NULL_ROW)).collect(),
            tracker: HeadPairTracker::new(),
        }
    }

    /// 追加一格，返回它的 id。
    pub(crate) fn add(&mut self, c: FakeCell) -> CellId {
        let id = CellId(self.cells.len() as u32);
        self.cells.push(c);
        id
    }

    /// 取格的 id（`add` 的顺序即 id 顺序）。
    pub(crate) fn id(&self, n: usize) -> CellId {
        CellId(n as u32)
    }

    /// `ds.getCurrent()`。
    pub(crate) fn cur(&self, ds: usize) -> Option<DsInstance> {
        self.ds[ds].value
    }
}

impl DsCurrentHost for Fake {
    fn cell00(&self) -> CellId {
        CellId::ROOT
    }

    fn left_head(&self, c: CellId) -> CellId {
        self.cells[c.0 as usize].left
    }

    fn top_head(&self, c: CellId) -> CellId {
        self.cells[c.0 as usize].top
    }

    fn is_extender(&self, c: CellId) -> bool {
        self.cells[c.0 as usize].extender
    }

    fn instance(&self, c: CellId) -> Option<DsInstance> {
        self.cells[c.0 as usize].inst
    }

    fn ext_cells(&self, c: CellId) -> Vec<CellId> {
        self.cells[c.0 as usize].ext.clone()
    }

    fn ds_count(&self) -> usize {
        self.ds.len()
    }

    fn ds_spot(&self, ds: usize) -> DsSpot {
        self.ds[ds]
    }

    fn ds_set_current(&mut self, ds: usize, v: Option<DsInstance>, is_left_top: bool) {
        self.ds[ds].set(v, is_left_top);
    }

    fn ds_root(&self, ds: usize) -> DsInstance {
        self.root[ds]
    }

    fn ds_null_row(&self, ds: usize) -> DsInstance {
        self.null[ds]
    }

    fn ds_row_cross(
        &mut self,
        ds: usize,
        recv: DsInstance,
        other: DsInstance,
    ) -> crate::Result<DsInstance> {
        row_cross_default(self, ds, recv, other)
    }

    fn tracker_set_changed(&mut self, changed: bool) {
        self.tracker.set_changed(changed);
    }
}
