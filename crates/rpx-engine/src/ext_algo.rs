//! 阶段 B 扩展算法（§12.1 第 8 步）：`extendRow` / `extendCol` 的逐行移植。
//!
//! 算法正文：`docs/format-contract.md` **附件 I**（I.1 调用点与 count 来源、
//! I.2 包络、I.3 插行/插列、I.5/I.6 两趟、I.7 对称性总表、I.8 expand*Head）；
//! 设计文档 `docs/m2-engine-design.md` §6。Java 源码 `ExtCellSet.java:981-1165`。
//!
//! # 结构对应
//!
//! | Java | Rust |
//! |------|------|
//! | `ExtCellSet._$26`（计算矩阵） | [`HeadState`] 的 `by_pos` + `row_size`/`col_size` |
//! | `ExtExtendCell._$6`（左子格列表） | `HeadCell.left_subs`（仅扩展格与 root 维护） |
//! | `ExtExtendCell._$5`（上子格列表） | `HeadCell.top_subs`（**扩列的施力点**，§6.3） |
//! | `ExtExtendCell._$7`（扩展实例数组） | `HeadCell.ext_cells`（`sub_cells` 的来源，§7） |
//! | `cell.clone()` | [`HeadState::clone_cell`] |
//!
//! # 移植硬约束（逐条有出处）
//!
//! 1. **`setCellLeftHead`/`setCellTopHead` 会向 head 登记子格**
//!    （`ExtNormalCell.java:357-376` → `head.addCellLeftHead(this)`），且**只有
//!    head 是扩展格时才登记**（`ExtCell.java:122-124` 基类空实现，仅
//!    `ExtExtendCell.java:100-117` 有实体）。head==self 报 `leftHeadCircle`
//!    （→ `CircularRef`）。`extendCol` 第 1 趟对 `leftHead==root` 的克隆是
//!    **直接 add**，不走 setCellLeftHead（不改指针，`ExtCellSet.java:1133-1135`）。
//! 2. **插列是「部分平移」**：`Matrix.insertCols(col,count,begin,end,align)`
//!    （`Matrix.java:253-281`）只平移带内行与（可选的）第 0 行，**带外行原位
//!    不动、新列留空** —— `engine_ext` 里 B1/C1/D1 被挤到 (1,6)/(1,7)/(1,8)
//!    的机制（探针实证，设计文档 §6.3）。插行（2 参版）才是全表平移。
//! 3. **clone 清空子格列表**（`ExtExtendCell.clone:48-54`），head 指针浅拷贝、
//!    由第 2 趟显式重写。`ext_cells` 置空 —— Java 是浅共享，但只有锚点克隆会
//!    被显式 `setExtCells` 重设；非锚点扩展格克隆不在任何夹具内（有意收敛，
//!    见 `row.rs` 模块文档）。
//! 4. **`count == 0` 不扩展直接返回 Ok**（附件 I.1c/I.10：上游保证 ≥1，产品靠
//!    `return` 回避 `ArrayIndexOutOfBoundsException`；Rust 侧不继承该假设）。
//! 5. `row_merge`/`col_merge` 按裸整数加减；Java 的 bit30/31 计算标志位不在
//!    M2 序列化对拍里（附件 K.1 要求 ③）。
//! 6. `_$8()` 中断检查（附件 I.4）在扩展循环里逐轮调用 —— 本模块暂无取消
//!    机制，顶层 `calculate()` 接线时在循环头补挂。
//!
//! # 已知边界（诚实声明）
//!
//! - 横向（19）扩展后克隆的**计算态行号归属**两夹具样本互相矛盾
//!   （附件 I.9），本实现只保证「同属一行 + 列走向」，不拟合行号。
//! - `_$3:653` 守卫等 head 链死代码问题与本模块无关（属 `heads/find.rs`）。

use crate::arena::CellId;
use crate::error::{EngineError, Result};
use crate::heads::HeadState;

mod col;
mod envelope;
mod expand;
mod insert;
mod row;

#[cfg(test)]
mod tests;

pub use col::extend_col;
pub use row::extend_row;

/// `isLeftHeadOf`（`ExtNormalCell.java:391-398`）：**自含**——从 `sub` 自身起步
/// 沿左 head 链上溯，命中 `anchor` 即真（`anchor.isLeftHeadOf(anchor) == true`，
/// 这正是锚点天然被克隆的原因，附件 I.5 易错点 1）。
/// 上限 4096 与 `heads/find.rs` 同源：归一化后的 head 链不会有环，防御性兜底。
pub(crate) fn is_left_head_of(g: &HeadState, anchor: CellId, sub: CellId) -> bool {
    let mut head = sub;
    for _ in 0..4096 {
        if head.is_none() {
            return false;
        }
        if head == anchor {
            return true;
        }
        head = g.cells[head.0 as usize].left_head;
    }
    false
}

/// `isTopHeadOf`（`ExtNormalCell.java:400-407`）：同上，沿上 head 链。
pub(crate) fn is_top_head_of(g: &HeadState, anchor: CellId, sub: CellId) -> bool {
    let mut head = sub;
    for _ in 0..4096 {
        if head.is_none() {
            return false;
        }
        if head == anchor {
            return true;
        }
        head = g.cells[head.0 as usize].top_head;
    }
    false
}

impl HeadState {
    /// `cell.clone()`（`ExtNormalCell.clone:133-149`、`ExtExtendCell.clone:48-54`）：
    /// 字段浅拷贝（head 指针原样共享），**子格列表清空**、`ext_cells` 置空。
    /// 新格**不落矩阵槽位**——由调用方 `fill_rect`/`set_pos` 铺放（与 Java 一致：
    /// clone 出的对象先游离，铺满动作才决定它坐哪）。
    ///
    /// # 克隆带走的 / 不带走的三处（逐条对应 Java）
    ///
    /// 1. ⛔ **`calculated` 必须保留**：`ExtNormalCell.clone:136` 写的是
    ///    `newCell.rowMerge &= Integer.MAX_VALUE`，抹的是 **bit31**（"已被要求求值"），
    ///    **bit30（"已求值"）被保留**。⇒ 克隆**继承「已求值」**，于是扩展出来的格
    ///    不会被通道 4/5 再算一遍（它们由锚点的第二趟写值循环 `ExtExtendCell.java:431-453`
    ///    直接写）。把这一位清掉会让每个克隆都被重新求值 ⇒ 值翻倍/串行，
    ///    且**不会有任何既有测试失败**（本轮之前 `calculate` 尚未接线）。
    /// 2. `demanded` 置回 `false`：同一句 `&= MAX_VALUE` 抹掉的 bit31 就是它。
    /// 3. `disp_override` 置 `None`：`ExtNormalCell.clone:135` 的 `newCell.propSetMap = null`。
    ///    ⇒ 克隆的 `getDispValue()` 回退到**模板声明**（`disp_declared`），而不是锚点的覆盖值。
    ///    值本身（`value`）**照抄**——`clone()` 不改 `cellValue`。
    /// 4. `instance`（`_$4`）置 `None`：`ExtExtendCell.clone:52` 的 `newCell._$4 = null`；
    ///    克隆由锚点第二趟按 `valList[i]` 逐个赋值。
    pub(crate) fn clone_cell(&mut self, src: CellId) -> CellId {
        let mut cell = self.cells[src.0 as usize].clone();
        cell.left_subs = Vec::new();
        cell.top_subs = Vec::new();
        cell.ext_cells = None;
        cell.disp_override = None;
        cell.rt.demanded = false;
        cell.rt.instance = None;
        let id = CellId(self.cells.len() as u32);
        self.cells.push(cell);
        id
    }

    /// `_$26.set(r, c, cell)`：把格放到计算矩阵槽位（不登记 `by_src` ——
    /// 引擎矩阵只认本体格，见 `grid.rs` 坐标系文档）。
    pub(crate) fn set_pos(&mut self, r: i32, c: i32, id: CellId) {
        self.by_pos.insert((r, c), id);
    }

    /// `_$26.get(r, c)`：槽位上的格；空槽 → [`CellId::NONE`]。
    pub(crate) fn at_pos(&self, r: i32, c: i32) -> CellId {
        self.by_pos.get(&(r, c)).copied().unwrap_or(CellId::NONE)
    }

    /// `_$1(startRow,startCol,endRow,endCol,head)`（`ExtCellSet.java:706-712`）：
    /// 合并块重铺 —— **半开区间** [r0,r1) × [c0,c1) 全部指向同一格。
    pub(crate) fn fill_rect(&mut self, r0: i32, c0: i32, r1: i32, c1: i32, id: CellId) {
        for r in r0..r1 {
            for c in c0..c1 {
                self.by_pos.insert((r, c), id);
            }
        }
    }

    /// `addRow(rowInc)`（`ExtNormalCell.java:200-202`）：只改自身行号。
    /// 合并块内的铺满槽位靠插行扫描的"跳块"跳过，**不逐格平移**（附件 I.3）。
    pub(crate) fn add_row(&mut self, id: CellId, inc: i32) {
        self.cells[id.0 as usize].row += inc;
    }

    /// `addCol(colInc)`（`:205-207`）。
    pub(crate) fn add_col(&mut self, id: CellId, inc: i32) {
        self.cells[id.0 as usize].col += inc;
    }

    /// `addRowMerge(inc)`（`:210-212`）：裸加法（硬约束 5）。
    pub(crate) fn add_row_merge(&mut self, id: CellId, inc: i32) {
        self.cells[id.0 as usize].row_merge += inc;
    }

    /// `addColMerge(inc)`（`:230-232`）。
    pub(crate) fn add_col_merge(&mut self, id: CellId, inc: i32) {
        self.cells[id.0 as usize].col_merge += inc;
    }

    /// `setExtCells`（`ExtExtendCell.java:118-120`）：登记扩展实例数组。
    /// 只对扩展格生效（方法只存在于 ExtExtendCell，普通格无此状态）。
    pub(crate) fn set_ext_cells(&mut self, id: CellId, members: &[CellId]) {
        if self.cells[id.0 as usize].is_extender {
            self.cells[id.0 as usize].ext_cells = Some(members.to_vec());
        }
    }

    /// `extCellSize()`（`:126-128`）：`None` 视作 1 —— M2 `sub_cells` 语义
    /// （设计文档 §7：17 格恒 1，18/19 = 扩展实例数）。
    /// 当前仅单测消费；顶层 `calculate()` 输出 `sub_cells` 时成为生产调用方。
    #[allow(dead_code)]
    pub(crate) fn ext_cell_size(&self, id: CellId) -> usize {
        match &self.cells[id.0 as usize].ext_cells {
            Some(v) => v.len(),
            None => 1,
        }
    }

    /// `setCellLeftHead`（`ExtNormalCell.java:357-367`）：写指针 + 向 head 登记。
    /// head==self → `leftHeadCircle`（→ `CircularRef`，M2-07 同类错误路径）；
    /// head 为空只清指针；**仅扩展格 head 接收登记**（硬约束 1）。
    pub(crate) fn set_cell_left_head(&mut self, id: CellId, head: CellId) -> Result<()> {
        if head == id {
            return Err(EngineError::CircularRef {
                chain: format!("cell#{}.leftHeadCircle", id.0),
            });
        }
        self.cells[id.0 as usize].left_head = head;
        if head.is_some() && self.cells[head.0 as usize].is_extender {
            self.cells[head.0 as usize].left_subs.push(id);
        }
        Ok(())
    }

    /// `setCellTopHead`（`:369-379`）：对称。
    pub(crate) fn set_cell_top_head(&mut self, id: CellId, head: CellId) -> Result<()> {
        if head == id {
            return Err(EngineError::CircularRef {
                chain: format!("cell#{}.topHeadCircle", id.0),
            });
        }
        self.cells[id.0 as usize].top_head = head;
        if head.is_some() && self.cells[head.0 as usize].is_extender {
            self.cells[head.0 as usize].top_subs.push(id);
        }
        Ok(())
    }
}
