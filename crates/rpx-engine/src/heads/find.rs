//! head 查找：为一个格找出它的**左 head** 与**上 head**（§12.1 第 5 步）。
//!
//! 算法正文见 `docs/format-contract.md` 附件 I 与 `docs/m2-engine-design.md` §4.2；
//! 本文件是 `ExtCellSet.java:631-704` 的**逐行**移植。
//!
//! # ⛔ 两个方向**故意不共用代码**（§12.1 易错点 5）
//!
//! 写成一份共用代码会同时踩四个坑：
//!
//! | 维度 | 左 head `_$3` | 上 head `_$2` |
//! |------|--------------|--------------|
//! | 扫描方向 | `col = cell.col-1` **递减**到 1（不含 0 号列） | `row = cell.row-1` **递减**到 1（不含 0 号行） |
//! | 行类型判定 | ⛔ **不调** `sameRowTypeAs` | ⛔ **调两次**（`:691` 命中分支 + `:695` 硬门槛） |
//! | 命中分支判的 extend | **18** | **19** |
//! | 命中分支的包含判定 | `containRow(cell)` | `sameRowTypeAs` + `containCol(cell)` |
//!
//! 真机判别：`engine_head_top` / `engine_head_left` 用 **行类型 `-96`** 做正反对照。
//! `sameRowTypeAs:266-268` 把 `{-91,-95,-94,-90}` 当**同族**，`-96` 不在内 ⇒ `:269` 走严格相等。
//! 于是「上 head 改判、左 head 不变」被实测观察到 —— 这是不对称存在的**证据**，不是推断。

use crate::arena::CellId;

/// 链遍历的防御性上限。Java 侧靠异常协程终止（`IIllIIIIIIIIIlll`），
/// Rust 侧必须自己设限，否则一条坏链就是死循环。
const CHAIN_LIMIT: usize = 4096;

/// head 查找所需的全部外部世界（对应 Java 的 `this`，即 `ExtCellSet`）。
///
/// 全部操作**以 `CellId` 寻址**，不直接暴露格数据 —— 这样 `find.rs` 不依赖
/// `CalcMatrix` 的内部布局，测试可注入一个纯内存的假实现。
pub trait HeadGrid {
    /// `cell00`（`_$10`）。
    fn cell00(&self) -> CellId;
    /// 当前格（`_$21`）。Java 用它排除「head-of-head 就是当前格」的情形。
    fn current(&self) -> CellId;
    /// 取**计算矩阵**（`_$26`）里 (row, col) 的格 id。越界或空格 ⇒ [`CellId::NONE`]。
    fn get_calc(&self, row: i32, col: i32) -> CellId;
    /// 取**引擎矩阵**里由格地址字符串（如 `"B2"`）解析出的格 id。
    /// 对应 `ExtCellSet.getSource(String)`（`:610-621`）。
    fn get_source_by_id(&self, id: &str) -> CellId;

    /// 该格的计算态行号。
    fn row(&self, id: CellId) -> i32;
    /// 该格的计算态列号。
    fn col(&self, id: CellId) -> i32;
    /// 是否参与合并（任一方向跨度 > 1）。
    fn is_merged(&self, id: CellId) -> bool;
    /// 生效的 extendMode（**已含** 16→17/18/19 的改写结果，见 `extend/role.rs`）。
    fn extend(&self, id: CellId) -> i8;
    /// 该格的纵向跨度是否覆盖 `row`（`ExtNormalCell.containRow(int):250-252`）。
    fn contain_row(&self, id: CellId, row: i32) -> bool;
    /// 该格的横向跨度是否覆盖 `col`（`ExtNormalCell.containCol(int):273-275`）。
    fn contain_col(&self, id: CellId, col: i32) -> bool;
    /// 该格的 `rowType`（行类型）。
    fn row_type(&self, id: CellId) -> i8;
    /// 两个格是否**行类型同族**（`ExtNormalCell.sameRowTypeAs:260-270`）：
    /// `type1` 落在同族集合 `{-91,-95,-94,-90}` 内则只比 `type2` 是否也在集合内；
    /// 否则走严格相等。⇒ **`-96` 与 `-91` 不同族**。
    fn same_row_type_as(&self, a: CellId, b: CellId) -> bool;

    /// 该格是否是**扩展格**（Java 的 `instanceof ExtExtendCell`）。
    ///
    /// 阶段 B 第 1 步的产物：有表达式且 `checkExtend` 为真的格被包成 `ExtExtendCell`
    /// （`docs/m2-engine-design.md` §4.1）。归一化（[`super::normalize`]）靠它判断
    /// 「是否已到达最近的扩展格」。
    ///
    /// ⚠ **`cell00` 必须返回 `true`** —— 它本身就是 `ExtExtendCell`（`:148`）。
    /// 否则归一化会沿链越过兜底格、拿到空链。
    fn is_extender(&self, id: CellId) -> bool;

    /// 该格的左 head（**引用**字段）。
    fn left_head(&self, id: CellId) -> CellId;
    /// 该格的上 head。
    fn top_head(&self, id: CellId) -> CellId;
    fn set_left_head(&mut self, id: CellId, head: CellId);
    fn set_top_head(&mut self, id: CellId, head: CellId);

    /// 该格**自声明**的左 head（模板里的 `getLeftHead()` **字符串**，`ExtNormalCell.java:1420-1421`）。
    ///
    /// ⚠ 这是与 [`HeadGrid::left_head`]**两个独立存储**：引用字段在阶段 B 开始时还是 null，
    /// 而字符串来自 `engine.srcPrepare`。Java 的**分支 A** 读字符串、
    /// **分支 B 与归一化**读写引用 —— **混成一体必错**。
    fn declared_left_head(&self, id: CellId) -> Option<String>;
    /// 同上，自声明的上 head。
    fn declared_top_head(&self, id: CellId) -> Option<String>;
}

/// head 查找失败（目前只有一种：自声明 head 成环）。
pub type HeadResult = Result<CellId, String>;

/// 找左 head —— `ExtCellSet._$3(ExtNormalCell)` `:631-666` 的逐行移植。
pub fn find_left_head<G: HeadGrid>(g: &mut G, cell: CellId) -> HeadResult {
    // ── 分支 A（:633-647）：模板自声明的 leftHead 字符串 ──
    if let Some(id) = non_blank(g.declared_left_head(cell)) {
        let head = g.get_source_by_id(&id);
        if !head.is_none() {
            if !is_left_head_of(g, cell, head) {
                return Ok(head);
            }
            return Err(circular(cell, "left"));
        }
    }

    // ── 分支 B（:648-664）：从 col-1 **递减**扫到 1，不含 0 号列 ──
    let xrow = g.row(cell);
    let mut col = g.col(cell) - 1;
    while col >= 1 {
        let head = g.get_calc(xrow, col);
        if head.is_none() {
            col -= 1;
            continue;
        }
        let extend = g.extend(head);
        // :653 合并格但 extend 不是 18、且它自己不在本格这一行 ⇒ 跳过
        if g.is_merged(head) && extend != 18 && g.row(head) != xrow {
            col -= 1;
            continue;
        }
        // :654-657 命中分支 —— ⛔ 不调 sameRowTypeAs，只 containRow
        if g.contain_row(head, g.row(cell)) && extend == 18 {
            if !is_left_head_of(g, cell, head) {
                return Ok(head);
            }
            col -= 1;
            continue;
        }
        // :658-661 head-of-head：仅当它**还没有**左 head 时才递归补上
        let hoh = g.left_head(head);
        if should_recurse(g, hoh, cell) && g.left_head(hoh).is_none() {
            let computed = find_left_head(g, hoh)?;
            g.set_left_head(hoh, computed);
        }
        // :662-663
        if hoh.is_none() || is_left_head_of(g, cell, hoh) {
            col -= 1;
            continue;
        }
        return Ok(hoh);
    }
    Ok(g.cell00()) // :665 兜底
}

/// 找上 head —— `ExtCellSet._$2(ExtNormalCell)` `:668-704` 的逐行移植。
///
/// ⛔ 与 [`find_left_head`] 的三处差异都是**契约**（见模块文档的对照表），不要"顺手统一"。
pub fn find_top_head<G: HeadGrid>(g: &mut G, cell: CellId) -> HeadResult {
    // ── 分支 A（:670-684） ──
    if let Some(id) = non_blank(g.declared_top_head(cell)) {
        let head = g.get_source_by_id(&id);
        if !head.is_none() {
            if !is_top_head_of(g, cell, head) {
                return Ok(head);
            }
            return Err(circular(cell, "top"));
        }
    }

    // ── 分支 B（:685-702）：从 row-1 **递减**扫到 1，不含 0 号行 ──
    let xcol = g.col(cell);
    let mut row = g.row(cell) - 1;
    while row >= 1 {
        let head = g.get_calc(row, xcol);
        if head.is_none() {
            row -= 1;
            continue;
        }
        let extend = g.extend(head);
        // :690 合并格但 extend 不是 19、且它自己不在本格这一列 ⇒ 跳过
        if g.is_merged(head) && extend != 19 && g.col(head) != xcol {
            row -= 1;
            continue;
        }
        // :691-694 命中分支 —— ⛔ 要 sameRowTypeAs，且 extend 判 **19**
        let same = g.same_row_type_as(cell, head);
        if same && g.contain_col(head, g.col(cell)) && extend == 19 {
            if !is_top_head_of(g, cell, head) {
                return Ok(head);
            }
            row -= 1;
            continue;
        }
        // :695 ⛔ **第二次** sameRowTypeAs 作硬门槛（左 head 没有这一步）
        if !same {
            row -= 1;
            continue;
        }
        let hoh = g.top_head(head);
        if should_recurse(g, hoh, cell) && g.top_head(hoh).is_none() {
            let computed = find_top_head(g, hoh)?;
            g.set_top_head(hoh, computed);
        }
        if hoh.is_none() || is_top_head_of(g, cell, hoh) {
            row -= 1;
            continue;
        }
        return Ok(hoh);
    }
    Ok(g.cell00()) // :703 兜底
}

// ==================== 共享小工具 ====================

/// `head != null && (head = head.trim()).length() > 0`（`:634`）。
fn non_blank(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// `should_recurse`（`:659` / `:697`）：
/// `hoh != null && != cell && != cell00 && != _$21 && hoh.getCellLeftHead() == null`
fn should_recurse<G: HeadGrid>(g: &G, hoh: CellId, cell: CellId) -> bool {
    !hoh.is_none() && hoh != cell && hoh != g.cell00() && hoh != g.current()
}

/// `cell.isLeftHeadOf(sub)`（`ExtNormalCell.java:391-398`）：从 `sub` **自身**起步沿链上溯。
/// ⚠ **自含**：传自身时返回 true —— 这正是"锚点自身被复制"能命中的原因。
fn is_left_head_of<G: HeadGrid>(g: &G, cell: CellId, sub: CellId) -> bool {
    let mut cursor = sub;
    for _ in 0..CHAIN_LIMIT {
        if cursor.is_none() {
            return false;
        }
        if cursor == cell {
            return true;
        }
        cursor = g.left_head(cursor);
    }
    false
}

fn is_top_head_of<G: HeadGrid>(g: &G, cell: CellId, sub: CellId) -> bool {
    let mut cursor = sub;
    for _ in 0..CHAIN_LIMIT {
        if cursor.is_none() {
            return false;
        }
        if cursor == cell {
            return true;
        }
        cursor = g.top_head(cursor);
    }
    false
}

/// 自声明 head 成环的诊断（`:638-646` / `:675-683`）。
///
/// ⚠ 只给**结构化**描述，不复刻产品那条本地化消息 —— 消息文本随语言变，
/// 对拍只取 `chain`。链的段数规则（`k` 环 → `k+2` 段）见 `format-contract.md` 附件 H.8。
fn circular(cell: CellId, axis: &str) -> String {
    format!("{axis} head 自声明成环：格 {} (CellId {})", cell.0, cell.0)
}