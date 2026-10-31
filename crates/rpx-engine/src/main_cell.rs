//! 主格判定（M2-01）。
//!
//! `MainCell.java:21-39` 的**忠实移植**。这一段决定"当前格归哪个主格管"，是扩展算法
//! 的地基：`changed == true` 表示这一格开启了新的主格分组，`false` 表示可以复用上一个
//! 主格对象（Java 侧就是复用同一个 `MainCell` 实例，所以这个布尔是有副作用的信号，
//! 不是纯查询）。
//!
//! 原码（注意 `&&` 优先于 `||`，且用的是**引用同一性**比较，不是 equals）：
//!
//! ```java
//! public void setCell(ExtCell cell, ExtCell cell00) {
//!     if (!this.changed) {
//!         ExtCell left = cell.getCellLeftHead();
//!         ExtCell top  = cell.getCellTopHead();
//!         if (left == cell00 && top == cell00 || left != this.leftHead) {
//!             this.leftHead = left; this.topHead = top; this.changed = true;
//!         } else if (top != this.topHead) {
//!             this.topHead = top; this.changed = true;
//!         } else {
//!             this.changed = false;
//!         }
//!     } else {
//!         this.leftHead = cell.getCellLeftHead();
//!         this.topHead  = cell.getCellTopHead();
//!     }
//! }
//! ```
//!
//! Rust 侧把"引用同一性"换成 [`CellId`] 的整数相等。`MainCell()` 的无参构造在 Java 里
//! `changed` 初值是 `true`（字段初始值），heads 是 `null` —— 这里照搬。

use crate::arena::CellId;

/// 主格分组判定器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainCell {
    left_head: CellId,
    top_head: CellId,
    changed: bool,
}

impl MainCell {
    /// 等价于 Java 的 `new MainCell()`：heads 为空，`changed` 初值 **true**。
    pub fn unset() -> Self {
        Self {
            left_head: CellId::NONE,
            top_head: CellId::NONE,
            changed: true,
        }
    }

    /// 等价于 Java 的 `new MainCell(left, top)`：`changed` 仍取字段初值 **true**。
    pub fn anchored(left: CellId, top: CellId) -> Self {
        Self {
            left_head: left,
            top_head: top,
            changed: true,
        }
    }

    /// 是否开启了一个新的主格分组。
    pub fn is_changed(&self) -> bool {
        self.changed
    }

    /// 显式改写 `changed`（Java `setChanged`）。
    pub fn set_changed(&mut self, changed: bool) {
        self.changed = changed;
    }

    /// 当前记录的 (leftHead, topHead)。
    pub fn heads(&self) -> (CellId, CellId) {
        (self.left_head, self.top_head)
    }

    /// Java `reset()`：两头清空，`changed` 置 `false`（注意与构造器相反）。
    pub fn reset(&mut self) {
        self.left_head = CellId::NONE;
        self.top_head = CellId::NONE;
        self.changed = false;
    }

    /// Java `setCell(cell, cell00)`。返回是否 `changed`（即是否要开新主格）。
    ///
    /// `cell00` 是矩阵原点格的 id；判定里对它的比较是**同一性**，不是"是否为空"。
    pub fn set_cell(&mut self, left: CellId, top: CellId, cell00: CellId) -> bool {
        if !self.changed {
            if (left == cell00 && top == cell00) || left != self.left_head {
                self.left_head = left;
                self.top_head = top;
                self.changed = true;
            } else if top != self.top_head {
                self.top_head = top;
                self.changed = true;
            } else {
                self.changed = false;
            }
        } else {
            self.left_head = left;
            self.top_head = top;
        }
        self.changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: CellId = CellId::ROOT;

    /// 分支 1/5：`changed == true` 时只搬运两头，不再做比较。
    #[test]
    fn while_changed_it_only_copies_the_heads() {
        let mut m = MainCell::unset(); // changed = true
        assert!(m.is_changed());
        // 即便两头与现值相同，也照搬（原码没有比较）
        assert!(m.set_cell(CellId(11), CellId(12), ROOT));
        assert_eq!(m.heads(), (CellId(11), CellId(12)));
    }

    /// 分支 2/5：`(left == cell00 && top == cell00)` 命中 → 两头都更新，开新主格。
    #[test]
    fn while_both_heads_are_cell00_it_opens_a_new_main_cell() {
        let mut m = MainCell::anchored(CellId(11), CellId(12));
        m.set_changed(false);
        assert!(m.set_cell(ROOT, ROOT, ROOT));
        assert_eq!(m.heads(), (ROOT, ROOT));
    }

    /// 分支 3/5：`left != leftHead` 命中 → 两头都更新。
    #[test]
    fn while_left_head_differs_both_heads_are_updated() {
        let mut m = MainCell::anchored(CellId(11), CellId(12));
        m.set_changed(false);
        assert!(m.set_cell(CellId(21), CellId(99), ROOT));
        assert_eq!(m.heads(), (CellId(21), CellId(99)));
    }

    /// 分支 4/5：只有 top 变 → 仅更新 top（left 保持不变）。
    #[test]
    fn while_only_the_top_head_differs_only_the_top_is_updated() {
        let mut m = MainCell::anchored(CellId(11), CellId(12));
        m.set_changed(false);
        assert!(m.set_cell(CellId(11), CellId(22), ROOT));
        assert_eq!(m.heads(), (CellId(11), CellId(22)));
    }

    /// 分支 5/5：两头都不变 → `changed = false`，可复用上一个主格对象。
    #[test]
    fn while_both_heads_match_it_reuses_the_previous_main_cell() {
        let mut m = MainCell::anchored(CellId(11), CellId(12));
        m.set_changed(false);
        assert!(!m.set_cell(CellId(11), CellId(12), ROOT));
        assert_eq!(m.heads(), (CellId(11), CellId(12)));
    }

    /// 关键陷阱：`left == cell00 && top == cell00` 的 **&&** 优先于 `||`。
    /// 只有 left 是 cell00、top 不是时，这一支**不**命中，要走后面的判断。
    #[test]
    fn cell00_arm_requires_both_heads_not_either() {
        let mut m = MainCell::anchored(CellId(11), CellId(12));
        m.set_changed(false);
        // left == cell00 但 top != cell00 → 落入第三支（left 与现值不同）而非第二支
        assert!(m.set_cell(ROOT, CellId(33), ROOT));
        assert_eq!(m.heads(), (ROOT, CellId(33)));

        // left == cell00 且 left 与现值相同、top 与现值相同 → 走最后一支，changed=false
        let mut n = MainCell::anchored(ROOT, CellId(12));
        n.set_changed(false);
        assert!(!n.set_cell(ROOT, CellId(12), ROOT));
    }

    /// `reset()` 把 `changed` 置 false —— 与两个构造器的 true 相反，容易写反。
    #[test]
    fn reset_differs_from_construction() {
        let mut m = MainCell::unset();
        assert!(m.is_changed());
        m.reset();
        assert!(!m.is_changed());
        assert_eq!(m.heads(), (CellId::NONE, CellId::NONE));
    }
}
