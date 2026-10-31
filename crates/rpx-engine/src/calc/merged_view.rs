//! 合并区的**两个视图**（M5-02）：全局逐槽 + 页内逐槽。
//!
//! # 为什么是两个视图（而不是一个）
//!
//! 产品侧「一份数据、两个读者」：
//!
//! * **对拍真值**（`CalcMergeJson.renderMerged`）：探针先跑
//!   `PageBuilder.createPages()` 再逐页读 `pg.getCell(r,c)` 的
//!   `getRowMerge()/getColMerge()` ⇒ 要的是**页内逐槽 + 页内跨度**
//!   （`cross_emp_bonus` 标题 9 列合并在第 1 页 8 列上，8 个槽全报 `col_merge=8`）；
//! * **服务面快照**（`rpx-server/src/snapshot`）：`geom::merge_data` 的矩形槽集
//!   不变式 + `paged::merge_data_paged` 的跨页 deepClone 拆分，吃的都是
//!   **全局逐槽 + 整段跨度**（页 2 的局部坐标若混进来会与页 1 碰撞）。
//!
//! 本模块从**同一份扫描**产出两份投影，两条消费链各取所需、互不污染。
//!
//! # 产品「拆分」= 本模块「求交」
//!
//! `SimplePager._$1` 对跨页合并做拆分：锚点区 `setEndRow`/`setEndCol` 收窄到页末，
//! 并 `deepClone` 出延续页新区（行向 `:1624-1638`、列向 `:2316-2335`）⇒ 后续页格
//! 拿到的是锚点值的克隆与**自己的**裁剪跨度。
//!
//! 等价建模：合并区矩形与页矩形求交，**交矩形维度**即该页所有相关槽报的跨度
//! （产品页格读的是合并区宽高，不是「本槽还剩多宽」）。交矩形 `1×1` 的页格被
//! 探针跳过（`CalcMergeJson.java:48-50`）⇒ 同跳（`cross_emp_bonus` 第 2 页标题槽
//! 即此形态）。
//!
//! # 已声明缺口（无夹具，不建模）
//!
//! 合并段断在**分组头行**（`rowType == -90`）时产品会把上一页合并区**伸长**盖住
//! （`SimplePager.java:1550-1596` 的 `rt == -90` 分支）—— 本模块只做矩形求交。
//! 现有夹具无此形态（跨页合并首例 `cross_emp_bonus` 是标题带，无分组头）。
//! 带账见 `docs/spec-m5.md` §6.3。

use crate::arena::{CalcGrid, CellId, MergedCell};
use crate::calc::ctx::CalcCtx;
use crate::pager::Pagination;
use crate::value::Value;

/// 一个合并区（产品 `MergedArea`）：锚点坐标 + 整段跨度 + 锚点值。
///
/// 锚点 = 该合并区在计算矩阵里**行主序首现**的槽位（`fill_rect` 把整个矩形
/// 写成同一个 [`CellId`]，所以首现槽即几何左上角；坐标取扫描序活槽位，
/// 与 `arena::to_grid` 同口径 —— arena 记录坐标可能不随 `insertRows` 更新）。
#[derive(Debug, Clone, PartialEq)]
pub struct MergedArea {
    /// 锚点行（全局，1 起）。
    pub r: i32,
    /// 锚点列（全局，1 起）。
    pub c: i32,
    /// 纵向跨度（`getRowMerge()`）。
    pub row_merge: i32,
    /// 横向跨度（`getColMerge()`）。
    pub col_merge: i32,
    /// 锚点值（延续槽共享同一 [`CellId`] ⇒ 同值）。
    pub value: Value,
}

impl MergedArea {
    /// 整段矩形（含端）：`[r, r+row_merge-1] × [c, c+col_merge-1]`。
    fn rect(&self) -> (i32, i32, i32, i32) {
        (
            self.r,
            self.r + self.row_merge - 1,
            self.c,
            self.c + self.col_merge - 1,
        )
    }
}

/// 扫**计算矩阵** `_$26`，按 [`CellId`] 去重提取全部合并区（`export_merged` 的内核）。
///
/// 只收 `rm > 1 || cm > 1` 的槽（`CalcMergeJson.java:48-50` 的判据在导出侧）；
/// **不跳空值格** ⇒ 锚点值可能为 [`Value::Null`]，`merged` 段因此比
/// `calculated.cells` 稠密。
pub fn areas(ctx: &CalcCtx) -> Vec<MergedArea> {
    let (row_size, col_size) = (ctx.grid.row_size, ctx.grid.col_size);
    let mut seen: Vec<CellId> = Vec::new();
    let mut out: Vec<MergedArea> = Vec::new();
    for r in 1..row_size {
        for c in 1..col_size {
            let Some(&id) = ctx.grid.by_pos.get(&(r, c)) else {
                continue;
            };
            if seen.contains(&id) {
                continue; // 合并区其余槽：同一对象，不重复成区
            }
            let h = &ctx.grid.cells[id.0 as usize];
            if h.row_merge <= 1 && h.col_merge <= 1 {
                continue;
            }
            seen.push(id);
            out.push(MergedArea {
                r,
                c,
                row_merge: h.row_merge,
                col_merge: h.col_merge,
                value: h.value.clone(),
            });
        }
    }
    out
}

/// **全局逐槽**视图（服务面快照的输入）：每个合并区的每个槽各一条，
/// 带**整段**跨度（延续槽报锚点跨度 —— `engine_samemerge` 的 4 行 = 2 锚点 × 2 槽）。
///
/// `page` 填占位 `1`：真页号只存在于页内视图（[`page_slots`]）。
pub fn global_slots(list: &[MergedArea]) -> Vec<MergedCell> {
    let mut out: Vec<MergedCell> = Vec::new();
    for a in list {
        let (r0, r1, c0, c1) = a.rect();
        for r in r0..=r1 {
            for c in c0..=c1 {
                out.push(MergedCell::new(
                    1,
                    r,
                    c,
                    a.value.clone(),
                    a.row_merge,
                    a.col_merge,
                ));
            }
        }
    }
    out
}

/// **页内逐槽**视图（对拍真值的形状）：每个合并区与每页矩形求交，交矩形内
/// **逐槽**发射（槽位置 → 页局部坐标，跨度 = 页内交矩形维度）。
///
/// 空页（`row_range == (0, 0)`，无区带也无数据行）跳过；交矩形 `1×1` 按
/// `CalcMergeJson.java:48-50` 跳过。未跑分页器的调用方不要用本函数 ——
/// 直接给 [`CalcGrid::merged`]（全局视图）即可。
pub fn page_slots(list: &[MergedArea], pg: &Pagination) -> Vec<MergedCell> {
    let mut out: Vec<MergedCell> = Vec::new();
    for page in &pg.pages {
        let (pr0, pr1) = page.row_range;
        let (pc0, pc1) = page.col_range;
        if pr0 == 0 && pr1 == 0 {
            continue; // 空页
        }
        for a in list {
            let (mr0, mr1, mc0, mc1) = a.rect();
            let (ir0, ir1) = (mr0.max(pr0), mr1.min(pr1));
            let (ic0, ic1) = (mc0.max(pc0), mc1.min(pc1));
            if ir0 > ir1 || ic0 > ic1 {
                continue; // 整段矩形不在本页
            }
            let rm = ir1 - ir0 + 1;
            let cm = ic1 - ic0 + 1;
            if rm <= 1 && cm <= 1 {
                continue; // 探针 `:48-50` 同跳
            }
            for r in ir0..=ir1 {
                for c in ic0..=ic1 {
                    out.push(MergedCell::new(
                        page.page,
                        r - pr0 + 1,
                        c - pc0 + 1,
                        a.value.clone(),
                        rm,
                        cm,
                    ));
                }
            }
        }
    }
    out
}

/// 把两个视图挂到导出网格上（`calc/run.rs::calculate_with` 的分页后接线）。
///
/// `merged` = 全局逐槽（快照映射吃）；`merged_pages` = 页内逐槽（emit 对拍吃）。
pub fn attach(grid: &mut CalcGrid, list: &[MergedArea], pg: &Pagination) {
    grid.merged = global_slots(list);
    grid.merged_pages = page_slots(list, pg);
}

#[cfg(test)]
mod tests;
