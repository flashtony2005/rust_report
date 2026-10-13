//! [`HeadGrid`] 的生产实现 [`HeadState`] —— 阶段 B 求值时承载 head 计算的可变格表。
//!
//! # 为什么是独立结构而不是直接挂在 `CalcMatrix` 上
//!
//! head 计算需要的字段（`extend` 改写值、`rowType`、合并跨度、声明式 head 字符串、
//! 左/上 head 引用）分属三个来源：引擎矩阵（改写与跨度）、模板行（rowType）、
//! 计算过程本身（head 引用）。`CalcCell` 只承载求值结果；把 head 状态塞进去
//! 会让求值结构背上调度状态。`HeadState` 是**阶段 B 的工作台**：由建格方
//! （阶段 A + 扩展算法）填充，head 归一化与求值调度共用。
//!
//! # 坐标系（⛔ 两套，别混）
//!
//! - **计算态 (row, col)**：`by_pos` 的键，扫描用（`getCell(xrow, col)`）。
//! - **引擎矩阵坐标（模板出处）**：`by_src` 的键，分支 A 解析声明式 head 字符串用
//!   （`getSource("B2")` 查的是**引擎矩阵**，不是计算矩阵）。
//!
//! 实测依据（`engine_head_left`）：计算格 `(5,1)` 是引擎 `(2,1)` 的克隆实例，
//! 但引擎矩阵在 `(5,1)` 上另有自己的格（模板 5 行）——两个矩阵在扩展后不再重合。
//!
//! # 已知边界
//!
//! - `cell00` 的 `row_type` 取 `-91`：它只会作为**兜底返回值**出现，永远不会被
//!   扫描命中（两个方向的扫描都止于 1，不含 0），所以该值不参与任何判定。
//!   若将来有路径让 `cell00` 进入 `sameRowTypeAs`，必须先回源码核实 0 号行的类型。

use std::collections::HashMap;

use super::cell::HeadCell;
use super::find::HeadGrid;
use crate::arena::CellId;

/// 同族行类型集合（`ExtNormalCell.sameRowTypeAs:266-268`）。
/// `-96`（表头）**不在**集合内 ⇒ 与 `-91` 判异，这是 `engine_head_top` 正反对照的判别式。
const ROW_TYPE_FAMILY: [i8; 4] = [-90, -91, -94, -95];

/// head 计算的工作台。见模块文档。
///
/// ⚠ 字段 `pub(crate)`：扩展算法（[`crate::ext_algo`]）需要直读直写
/// `by_pos`/`cells`/尺寸计数，trait（[`HeadGrid`]）那层只服务查找与归一。
#[derive(Debug)]
pub struct HeadState {
    pub(crate) cells: Vec<HeadCell>,
    /// 计算态坐标 → 格 id（`getCell(row, col)` ＝ `_$26` 计算矩阵）。
    pub(crate) by_pos: HashMap<(i32, i32), CellId>,
    /// 引擎矩阵坐标 → 格 id（`getSource("B2")` 的替身，`_$25`）。
    pub(crate) by_src: HashMap<(i32, i32), CellId>,
    /// 当前正在处理的格（Java 的 `_$21`）；未设置时为 [`CellId::NONE`]。
    pub(crate) current: CellId,
    /// 矩阵行数（**含** 0 号脚手架；`_$26.getRowSize()`，与 `_$16` 同口径）。
    pub(crate) row_size: i32,
    /// 矩阵列数（含 0 号；与 `_$15` 同口径）。
    pub(crate) col_size: i32,
}

impl Default for HeadState {
    fn default() -> Self {
        HeadState {
            cells: Vec::new(),
            by_pos: HashMap::new(),
            by_src: HashMap::new(),
            current: CellId::NONE,
            row_size: 0,
            col_size: 0,
        }
    }
}

impl HeadState {
    /// 空表。**第 0 个推入的格自动成为 `cell00`**（[`CellId::ROOT`]）——
    /// 与 `stage_a` 的建格顺序一致：`(0,0)` 先建。
    pub fn new() -> Self {
        HeadState::default()
    }

    /// 推入一格，返回其 id。同时按计算态坐标建索引，并让尺寸计数**至少**容纳
    /// 该格（含 0 号行/列的口径：row+1 / col+1 的最大值）。
    pub fn push(&mut self, cell: HeadCell) -> CellId {
        let id = CellId(self.cells.len() as u32);
        self.by_pos.insert((cell.row, cell.col), id);
        self.row_size = self.row_size.max(cell.row + 1);
        self.col_size = self.col_size.max(cell.col + 1);
        self.cells.push(cell);
        id
    }

    /// 推入一个**影子格**：只进 `cells`，**不进 `by_pos`**、不占槽位。
    ///
    /// 对应产品 `_$25` 引擎矩阵里**纵向合并延续槽**上的独立引擎
    /// （`ExtCellSet.java:166-192` 构造循环对纵向槽的正常覆盖；2026-10-13
    /// 产品 JRE 实证：`getSource(3,1)` → src=(3,1) rm=1 cm=1 ext=16 的独立格，
    /// 而 `getCell(3,1)` 是锚点别名 —— 两张矩阵分歧）。
    ///
    /// 影子格只服务两件事：导出的 `source` 段、`getSource("A3")` 型地址查询。
    /// 它**不进** `by_pos` ⇒ 通道扫描（`slot_matches`）、head 归一化、
    /// `prepareCalculate` 的主格判定都摸不到它 —— 与产品的独立引擎一致
    /// （引擎没有表达式、不参与计算）。
    pub fn push_shadow(&mut self, cell: HeadCell) -> CellId {
        let id = CellId(self.cells.len() as u32);
        self.cells.push(cell);
        id
    }

    /// 把 `id` 登记到引擎矩阵坐标 `src` 上（分支 A 的地址查表）。
    /// 同一坐标重复登记以**最后一次**为准 —— 调用方应只对引擎矩阵本体格登记，
    /// 克隆实例不登记（Java 的 `getSource(String)` 只认引擎矩阵）。
    pub fn index_src(&mut self, src: (i32, i32), id: CellId) {
        self.by_src.insert(src, id);
    }

    /// 设置当前处理格（每个格开始求值前调用一次）。
    pub fn set_current(&mut self, id: CellId) {
        self.current = id;
    }

    /// 取格属性（越界 id 返回 `None`）。
    pub fn get(&self, id: CellId) -> Option<&HeadCell> {
        self.cells.get(id.0 as usize)
    }

    /// `cell00` 的 id（恒为 [`CellId::ROOT`]，前提：构造顺序遵守模块文档）。
    pub fn root(&self) -> CellId {
        CellId::ROOT
    }

    /// 引擎出处坐标 `(sourceRow, sourceCol)`（见 [`HeadCell::src_row`]）。
    pub fn src_of(&self, id: CellId) -> (i32, i32) {
        let c = &self.cells[id.0 as usize];
        (c.src_row, c.src_col)
    }

    /// `cell.getSource()`（`ExtNormalCell.getSource():151-153` → `engine.srcCurrent`）：
    /// 该格的**引擎矩阵本体格**。克隆实例返回它克隆自的那一格。
    ///
    /// 实现方式：按 [`HeadCell::src_row`] 查 `by_src`（引擎矩阵地址表）。
    /// ⛔ 不能拿 `row`/`col` 查 —— 扩展后克隆的运行时坐标不再等于模板坐标
    /// （`engine_ext` 里模板 `(1,1)` 的克隆会坐到 `(2,2)`）。
    /// `cell00` 的出处是它自己（`by_src` 不含 `(0,0)`，故单列）。
    pub fn source_of(&self, id: CellId) -> CellId {
        if id.is_none() {
            return CellId::NONE;
        }
        if id == CellId::ROOT {
            return CellId::ROOT;
        }
        let (r, c) = self.src_of(id);
        self.by_src.get(&(r, c)).copied().unwrap_or(CellId::NONE)
    }
}

/// 地址解析：`"B2"` / `"$B$2"` → 计算矩阵坐标 `(2, 2)`。
/// 字母段 = 列（A=1），数字段 = 行。只认大写？产品用 `equalsIgnoreCase` 语义，
/// 这里统一转大写。空串或无数字段返回 `None`。
fn parse_address(s: &str) -> Option<(i32, i32)> {
    let s: String = s.chars().filter(|c| *c != '$').collect();
    let mut letters = String::new();
    let mut digits = String::new();
    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            if !digits.is_empty() {
                return None; // 字母出现在数字之后：不是合法地址
            }
            letters.push(c.to_ascii_uppercase());
        } else if c.is_ascii_digit() {
            digits.push(c);
        } else {
            return None;
        }
    }
    if letters.is_empty() || digits.is_empty() {
        return None;
    }
    let mut col = 0i32;
    for c in letters.chars() {
        col = col * 26 + (c as i32 - 'A' as i32 + 1);
    }
    let row: i32 = digits.parse().ok()?;
    Some((row, col))
}

impl HeadGrid for HeadState {
    fn cell00(&self) -> CellId {
        CellId::ROOT
    }

    fn current(&self) -> CellId {
        self.current
    }

    fn get_calc(&self, row: i32, col: i32) -> CellId {
        self.by_pos.get(&(row, col)).copied().unwrap_or(CellId::NONE)
    }

    fn get_source_by_id(&self, id: &str) -> CellId {
        match parse_address(id) {
            Some(src) => self.by_src.get(&src).copied().unwrap_or(CellId::NONE),
            None => CellId::NONE,
        }
    }

    fn row(&self, id: CellId) -> i32 {
        self.cells[id.0 as usize].row
    }

    fn col(&self, id: CellId) -> i32 {
        self.cells[id.0 as usize].col
    }

    fn is_merged(&self, id: CellId) -> bool {
        let c = &self.cells[id.0 as usize];
        c.row_merge > 1 || c.col_merge > 1
    }

    fn extend(&self, id: CellId) -> i8 {
        self.cells[id.0 as usize].extend
    }

    fn contain_row(&self, id: CellId, row: i32) -> bool {
        // `ExtNormalCell.containRow(int):250-252`：从**自身行**起、跨 `rowMerge` 行。
        let c = &self.cells[id.0 as usize];
        row >= c.row && row < c.row + c.row_merge
    }

    fn contain_col(&self, id: CellId, col: i32) -> bool {
        let c = &self.cells[id.0 as usize];
        col >= c.col && col < c.col + c.col_merge
    }

    fn row_type(&self, id: CellId) -> i8 {
        self.cells[id.0 as usize].row_type
    }

    fn same_row_type_as(&self, a: CellId, b: CellId) -> bool {
        let t1 = self.cells[a.0 as usize].row_type;
        let t2 = self.cells[b.0 as usize].row_type;
        if ROW_TYPE_FAMILY.contains(&t1) {
            ROW_TYPE_FAMILY.contains(&t2)
        } else {
            t1 == t2
        }
    }

    fn is_extender(&self, id: CellId) -> bool {
        self.cells[id.0 as usize].is_extender
    }

    fn left_head(&self, id: CellId) -> CellId {
        self.cells[id.0 as usize].left_head
    }

    fn top_head(&self, id: CellId) -> CellId {
        self.cells[id.0 as usize].top_head
    }

    /// ⛔ `setCellLeftHead` **不只是写指针**：`head.addCellLeftHead(this)`
    /// （`ExtNormalCell.java:357-367`）会向 head 登记子格，且**只有扩展格 head 收登记**
    /// （基类空实现 `ExtCell.java:122-124`）—— 漏登会让 `cell00._$6` 少格（§6.3 的 12 条）。
    /// `find.rs:660`/`normalize.rs` 调的就是这个带副作用的 setter；自环由阶段 B 驱动报出。
    fn set_left_head(&mut self, id: CellId, head: CellId) {
        self.cells[id.0 as usize].left_head = head;
        if head.is_some() && head != id && self.cells[head.0 as usize].is_extender {
            self.cells[head.0 as usize].left_subs.push(id);
        }
    }

    /// 同左（登记进 `_$5`）。
    fn set_top_head(&mut self, id: CellId, head: CellId) {
        self.cells[id.0 as usize].top_head = head;
        if head.is_some() && head != id && self.cells[head.0 as usize].is_extender {
            self.cells[head.0 as usize].top_subs.push(id);
        }
    }

    fn declared_left_head(&self, id: CellId) -> Option<String> {
        self.cells[id.0 as usize].declared_left.clone()
    }

    fn declared_top_head(&self, id: CellId) -> Option<String> {
        self.cells[id.0 as usize].declared_top.clone()
    }
}
