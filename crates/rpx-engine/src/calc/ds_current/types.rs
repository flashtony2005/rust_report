//! DS current 的数据类型（自 `ds_current.rs` 拆出，门禁行数）。
//!
//! 三个类型各自对应 Java 的一个小对象：
//! [`DsInstance`] ↔ `DsValue`（实例）、[`DsSpot`] ↔ `DataSet` 的 `current` 三件套、
//! [`HeadPairTracker`] ↔ `lIlIlllIIllllIII`（`ExtCellSet._$19`）。
//! 语义与出处见父模块的模块文档。

use crate::arena::CellId;
use crate::expr::{ds::DsCur, eval::NULL_ROW_NOTE};

/// `getNullRow()`（`DataSet.java:82`）：值全空的哨兵行（`Row.cross` 的失败返回）。
pub const NULL_ROW: usize = usize::MAX;

/// 编译期钉子：expr 侧旁路哨兵（`note_ds_row(ds, NULL_ROW_NOTE)`，
/// M5-01 `ds.select` Row 态过滤未命中）与本常量**必须同值** ——
/// `mirror` 把它投影成 `DsCur::Row(NULL_ROW)`、`is_real_row` 按它分流，
/// 漂移会让空行身份静默变成真实行（或反向）。
const _: () = assert!(NULL_ROW == NULL_ROW_NOTE);

/// `_$4`：一个扩展格"当前代表的那个 `DsValue` 实例"。
///
/// `ds` 是数据集的稳定下标；`current` 是行、组或根；行号 [`NULL_ROW`] 表示空行。
/// 产品的 `DsValue` 还带一个求值缓存（`row.setValue(...)`），本 crate 里对应
/// [`crate::expr::DataSet::selected`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DsInstance {
    /// 数据集下标（调用方维护 name → 下标的映射）。
    pub ds: usize,
    /// 当前行或组的稳定身份。
    pub current: DsCur,
}

impl DsInstance {
    /// 一行实例。
    pub fn new(ds: usize, row: usize) -> Self {
        DsInstance {
            ds,
            current: DsCur::Row(row),
        }
    }

    pub fn member(ds: usize, current: DsCur) -> Self {
        Self { ds, current }
    }

    /// 该数据集的根 Group（全量）。
    pub fn root(ds: usize) -> Self {
        DsInstance {
            ds,
            current: DsCur::Root,
        }
    }

    /// 是否是根 Group。
    pub fn is_root(&self) -> bool {
        self.current == DsCur::Root
    }

    /// 是否是真的某一行（不是根、也不是空行）。
    pub fn is_real_row(&self) -> bool {
        matches!(self.current, DsCur::Row(row) if row != NULL_ROW)
    }
}

/// 一个数据集的 `current` 三件套（`DataSet.java:358-366`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DsSpot {
    /// `_$6`：当前 DsValue；`None` = Java 的 `null`（`resetDSCurrent` 的产物）。
    pub value: Option<DsInstance>,
    /// `_$4`：`isLeftTop` 标志 —— 由**最近一次** `setCurrent(g, flag)` 写入。
    /// `isLeftTop()`（`:336-338`）读的就是它，**不是**数据集的固有属性。
    pub is_left_top: bool,
    /// `_$5`：`currentRow`。`setCurrent(g)` 每次都置 1（`:365`）；
    /// `restoreCurrent(g, r)`（`:378-381`）可写别的值。
    pub current_row: i32,
}

impl DsSpot {
    /// 初始态：没有 current，标志假，行号 0。
    pub fn empty() -> Self {
        DsSpot {
            value: None,
            is_left_top: false,
            current_row: 0,
        }
    }

    /// `setCurrent(g, isLeftTop)`（`:358-366`）：写 `_$6`/`_$4`，`_$5` 一律复位成 1。
    pub fn set(&mut self, v: Option<DsInstance>, is_left_top: bool) {
        self.value = v;
        self.is_left_top = is_left_top;
        self.current_row = 1;
    }
}

/// `lIlIlllIIllllIII`（`ExtCellSet._$19`）：`(leftHead, topHead)` 对变化跟踪器。
///
/// 语义 = 「本格与上一格的 `(leftHead, topHead)` 对不同」（外加"两者都是 `cell00`"
/// 视作变化）。构造时 `changed = true`；[`HeadPairTracker::reset`] 置 `false` 并清空记录。
#[derive(Debug, Clone, Copy)]
pub struct HeadPairTracker {
    left: CellId,
    top: CellId,
    changed: bool,
}

impl Default for HeadPairTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadPairTracker {
    /// `_$1 = true`（Java 构造时的初值）。
    pub fn new() -> Self {
        HeadPairTracker {
            left: CellId::NONE,
            top: CellId::NONE,
            changed: true,
        }
    }

    /// `setCell(cell, cell00)`（`lIlIlllIIllllIII.java:21-39`）。
    ///
    /// ⛔ 顺序：**先判 `changed`**。为真时只更新记录并保持真（不比较）。
    pub fn set_cell(&mut self, left: CellId, top: CellId, cell00: CellId) {
        if self.changed {
            self.left = left;
            self.top = top;
            return;
        }
        if (left == cell00 && top == cell00) || left != self.left {
            self.left = left;
            self.top = top;
            self.changed = true;
        } else if top != self.top {
            self.top = top;
            self.changed = true;
        }
        // 否则保持 false —— 与上一格同属一个 head 区，DS 指针无需重算
    }

    /// `setChanged(boolean)`（`:41-43`）。
    pub fn set_changed(&mut self, changed: bool) {
        self.changed = changed;
    }

    /// `isChanged()`（`:45-47`）＝ `ExtCellSet.isMainChanged()`（`:550-552`）。
    pub fn is_changed(&self) -> bool {
        self.changed
    }

    /// `reset()`（`:49-53`）：`_$3 = _$2 = null; _$1 = false`。
    pub fn reset(&mut self) {
        self.left = CellId::NONE;
        self.top = CellId::NONE;
        self.changed = false;
    }
}
