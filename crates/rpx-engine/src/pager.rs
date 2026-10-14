//! 分页器（M2-11）：`SimplePager.createPages()` 的移植。
//!
//! # Java 事实源与算术（reportlite，2026-10-13）
//!
//! `ReportUtils2.getPagerInfo`（`:104-125`）覆盖纸张、清零边距；`PagerInfo` 默认行数 20、按高断页（`:239-281`）。
//! 换算见 `Pager.unitTransfer`（`Pager.java:160-172`）与 `PrintSetup.mmToPixel`（`:1150-1152`）；
//! 主循环 `SimplePager._$5`（`:274-`），纵向判据/累高 `_$1`/`_$4`（`:1493-1520`），横断 `_$2`（`:1836-1953`、`:2354-2374`）。
//! 算术钉子：纸张 pt→mm→pt 经 float32（842→841.9999730868603）；高精度默认 true 不逐行 ceil；纵横超界判据均为 `cost > ceil((limit-used)*1000)/1000`。
//!
//! # 两种页尺寸口径（M3-06，**算法只有一份**）
//!
//! | 口径 | 入口 | 页尺寸 | 消费者 |
//! | --- | --- | --- | --- |
//! | 探针 | [`paginate`]（`calculate()` 内） | [`PAPER_W_PT`]/[`PAPER_H_PT`] 常量 595×842pt、零边距、`COLUMNS`=1 | 夹具 `pagination` 段对拍（M2-11 立的基准） |
//! | 模板真值 | [`paginate_with_setup`]（`/render` 接线） | [`paper::usable_page_pt`]：纸码表 + 模板边距 | 快照 `pagination` 块（`spec-m3.md` §3.2/M3-06） |
//!
//! 探针口径是**产品探针自己覆盖出来的**（`ReportUtils2.getPagerInfo` 把纸张改成
//! 传入的 595×842 并清零边距，`setPaperSize` 置纸码 256=自定义）——夹具的
//! `pagination` 段记的就是它。两条口径共用 [`split_rows`]/[`split_cols`]，
//! 差只在页尺寸与 `columns`/`row_num_per_page` 的来源。为何不合并成一条：
//! 夹具是 M2-11 的对拍基准（探针路径），改了它 17 个夹具全红；而 `/render`
//! 要的是产品真实路径（模板 `PrintSetup` 原值）。两份口径都有明确的 Java
//! 事实源，不是漂移。
//!
//! # 边界（诚实声明）
//!
//! **已实现且与夹具逐页对上**：
//!
//! * 纵向按行高断页、数据区范围（`_$2(r)` 的首/末 `-91`/`-90` 行）；
//! * **页首区带每页重复**：页眉 `-93` / 标题 `-96` / 表头 `-95`（`_$4(r, t)` 的首个连续段），
//!   以及页脚区带 `-92`/`-89`/`-94`（`_$3(r, t)` 的末个连续段）的**占高**；
//! * 分组头（`rowType == -90`）在每页顶部重复（副本**也吃页高**、也计入该页 `rows`）；
//! * 横向按列宽贪心断页；
//! * 每页的 `rows` 与**首列**非空值序列。
//!
//! 12 个网格夹具里 **11 个**参与分页对拍，且全部逐页一致：`engine_pager` 4 页、
//! `engine_sum` 2 页、`engine_rowheight` 2 页，其余 8 个单页。`engine_rowheight` 的
//! 计算态行高由 M2-22 的 RowCell key 1 表达式得到，切点为 8+9 行。
//!
//! **未实现（逐条说明为什么现在不做）**：
//!
//! * **`pagerStyle == 1`（按行数断页）与 `== 0`（单页短路）** —— 夹具全是默认 `2`；
//!   2026-10-14 起有显式守卫（[`ensure_supported`]，`guard.rs`），不再静默按 `2`。
//! * **跨页合并格拆分**（`_$1(row, currPageRows, …):1536-1700`）—— 夹具的合并区都不跨页
//!   （多页夹具 `merged.cells` 全空）。
//! * **行/列不可见与手动分页符** —— 属性表达式先由 `calcProperties` 求值；不可见行仍
//!   保留在页行序但不计高，列宽记零；row/col `breakPage` 在对应轴强制断页。
//! * **列区带**（左侧列头 `_$4(currPageRows)` / 右侧列头 `_$3(currPageRows)`）——
//!   夹具的 `colType` 全是 `-79`（数据列），左右区带恒为 `null`；故数据列范围固定取
//!   `1..=cols`。2026-10-14 起有显式守卫（[`ensure_data_columns`]）。
//! * **文本自动换行回填行高**（`_$1(…):1557-1573` 的 `-64`/`-56` 分支）—— ⛔ **无守卫
//!   也不可有**：`-64` 是 `INormalCell.TYPE_NORMAL`（普通格默认类型，任何模板都有），
//!   测量分支只在「值为 `String` 且页内放不下」时触发——两个条件都是断页时才知道，
//!   静态拦截会拒绝所有真实模板。核对结论与带账见 `guard.rs` 尾注（M3-06）。
//! * **多栏布局**（`columns > 1`、`columnMode == 1`、`isColumnRepeatedGroupHeader`）——
//!   探针固定传 `columns = 1`。⚠ 同受守卫（`columnMode` / 重复组头 / `tableColumnNum` 非默认即报）。
//!
//! # 行高的来源（与 M2-22 的关系）
//!
//! 产品的 `report.getRowCell(row)` 在**扩展后**返回的是**计算态行**。本 crate 先按扩展映射铺开
//! 模板行高，再以 `calcProperties` 写回值覆盖；模板列宽与列属性同理。`engine_rowheight` 的
//! 17 个计算态行高与产品夹具逐值一致，见 `tests/engine_rowheight_pager.rs`。

mod arith;
mod guard;
mod paper;
mod split;

// `fits` 只被本模块的测试用（算法在 split.rs）—— cfg(test) 引入，免 unused。
#[cfg(test)]
use arith::fits;
pub use arith::{paper_size_pt, unit_to_pt};
pub use guard::{ensure_data_columns, ensure_supported};
pub use paper::{paper_height_mm, paper_width_mm, usable_page_pt};
pub use split::{
    data_rows, leading_run, split_cols, split_rows, trailing_run, PageRow, PageRowKind, YSplit,
    ROW_TYPE_GROUP_HEADER,
};

use crate::arena::CalcGrid;
use crate::error::{EngineError, Result};
use crate::value::Value;
use rpx_core::print_setup::PrintSetup;

/// 探针的纸张口径：`CalcJson.renderPages(ecs, 595, 842, 1)` 的三个实参。
///
/// ⚠ **不是**报表自己的 `PrintSetup`：产品侧 `ReportUtils2.getPagerInfo` 会用
/// `info.setPaperSize(...)` **覆盖**纸张、把四边边距**清零**（`ReportUtils2.java:107-115`）。
/// 夹具因此记的是探针的 595×842 + 零边距，而不是模板里的 paper=9 + 19/19/25/25mm。
/// 本 crate 复刻探针口径，否则与 `expected.json` 的 `paper_w/paper_h` 对不上。
pub const PAPER_W_PT: f32 = 595.0;
/// 纵向纸张（[`PAPER_W_PT`] 同款理由）。
pub const PAPER_H_PT: f32 = 842.0;
/// 横向分栏数（探针实参 `columns = 1`）。
pub const COLUMNS: i32 = 1;
/// `PagerInfo.rowNumPerPage` 的默认值 —— **只回显，不参与断页**（`pagerStyle == 2`）。
pub const ROW_NUM_PER_PAGE: i32 = 20;

/// 一页的观测值（`CalcJson.renderPages` 的 `po` 对象，`tests/fixtures/CalcJson.java:165-175`）。
#[derive(Debug, Clone, PartialEq)]
pub struct PageInfo {
    /// 页号（1 起）。
    pub page: i32,
    /// 该页行数（`pg.getRowCount()`）—— **含分组头副本**，也**不等于**
    /// [`Self::col1_values`] 的长度（空值被跳过）。
    pub rows: i32,
    /// 该页**首列**的全部非空值（按行序）。
    ///
    /// ⛔ 横向翻页后「首列」是该页自己的第 1 列（`pg.getCell(r, 1)`），
    /// 不是网格第 1 列 —— `engine_sum` 第 2 页的 `ds2Count` 就是第 9 列的值。
    ///
    /// ⛔ `first_value` / `last_value` **不在这里存**：它们就是本序列的首/末元素
    /// （探针同样从这一遍扫描里取），存两份迟早会漂移。导出期由
    /// [`crate::emit`] 用 `String.valueOf` 补出（判等口径与 `value()` 不同，
    /// 见 `emit/val.rs` 的「两个函数，别混用」）。
    pub col1_values: Vec<Value>,
    /// 该页在网格里的**行范围**（1 起，含端；页首区带与分组头副本计入）。
    ///
    /// M3-06 新增，**只服务快照 `pagination` 块**（`spec-m4.md` §4）：夹具对拍
    /// 不读它（`emit` 逐键序列化，见 `emit/pagination.rs`），故加字段不影响
    /// M2-11 立的基准。空页（无区带也无数据行）记 `(0, 0)`。
    pub row_range: (i32, i32),
    /// 该页的**列范围**（1 起，含端）—— 横向断页的 `(首列, 末列)`。
    pub col_range: (i32, i32),
}

/// `expected.json` 的 `pagination` 段（键序见 `CalcJson.java:177-185`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Pagination {
    /// 该口径下的纸张宽（pt）：探针口径 = 传入的 [`PAPER_W_PT`]；
    /// 模板真值口径 = [`usable_page_pt`] 的可用宽（`/render` 用）。
    pub paper_w: f32,
    /// 该口径下的纸张高（pt）（[`Self::paper_w`] 同款双口径）。
    pub paper_h: f32,
    /// 横向分栏数。
    pub columns: i32,
    /// 总页数 = `x_pages * y_pages`（`PageBuilder.getPageCount()`）。
    pub page_count: i32,
    /// 横向页数。
    pub x_pages: i32,
    /// 纵向页数。
    pub y_pages: i32,
    /// `PagerInfo.rowNumPerPage`（默认 20）。⛔ 不参与断页。
    pub row_num_per_page: i32,
    /// 逐页观测值，序为 **y 外层、x 内层**（`PageBuilder.getAllPages():291-305`）。
    pub pages: Vec<PageInfo>,
}

impl Default for Pagination {
    /// 单页兜底（分页器未跑时的形状）。
    fn default() -> Self {
        Pagination {
            paper_w: PAPER_W_PT,
            paper_h: PAPER_H_PT,
            columns: COLUMNS,
            page_count: 1,
            x_pages: 1,
            y_pages: 1,
            row_num_per_page: ROW_NUM_PER_PAGE,
            pages: Vec::new(),
        }
    }
}

impl Pagination {
    /// 页数（`merged.page_count` 与 `pagination.page_count` 同意）。
    pub fn page_count(&self) -> i32 {
        self.page_count
    }
}

/// 分页主入口：纵向断页 → 每纵向页再横向断页。
///
/// 页序 **y 外层、x 内层**（`PageBuilder.getAllPages():296-302` 的 `pages[i * x + j]`）。
///
/// `columns` 只作回显（本实现只覆盖 `pagerStyle == 2 && columns == 1` 分支，
/// 其余由 [`ensure_supported`] 拦住）。
pub fn paginate(grid: &CalcGrid, paper_w_pt: f32, paper_h_pt: f32, columns: i32) -> Pagination {
    let page_h = paper_size_pt(paper_h_pt);
    let page_w = paper_size_pt(paper_w_pt);
    let col_spans = split_cols(grid, page_w);
    let x_pages = col_spans.len() as i32;
    let ys = split_rows(grid, page_h);
    let y_pages = ys.pages.len() as i32;

    let mut out = Pagination {
        paper_w: paper_w_pt,
        paper_h: paper_h_pt,
        columns,
        page_count: x_pages * y_pages,
        x_pages,
        y_pages,
        row_num_per_page: ROW_NUM_PER_PAGE,
        pages: Vec::new(),
    };
    let mut page_no = 0;
    for page_rows in &ys.pages {
        let row_range = match (page_rows.first(), page_rows.last()) {
            (Some(f), Some(l)) => (f.row, l.row),
            _ => (0, 0),
        };
        for &(c0, c1) in &col_spans {
            page_no += 1;
            out.pages.push(PageInfo {
                page: page_no,
                rows: page_rows.len() as i32,
                col1_values: split::col_values(grid, page_rows, c0),
                row_range,
                col_range: (c0, c1),
            });
        }
    }
    out
}

/// 模板真值口径分页（M3-06）：页尺寸来自 [`paper::usable_page_pt`]
/// （纸码表 + 模板边距，`PagerInfo.getPageFormat(true)` 的 imageable 区）。
///
/// 与 [`paginate`] 共用 [`split_rows`]/[`split_cols`]；差在：
/// 页尺寸（可用区而非探针常量）、`columns`/`row_num_per_page` 取模板值
/// （`tableColumnNum` 已由 [`ensure_supported`] 确保为 1）。
///
/// 自带 [`ensure_supported`] 复核（本函数是 pub API，调用方可能没经过
/// `calculate()` 入口的守卫）与可用尺寸非正检查（边距 ≥ 纸张 ⇒ 显式失败，
/// 不静默产出荒谬页）。
pub fn paginate_with_setup(grid: &CalcGrid, ps: &PrintSetup) -> Result<Pagination> {
    ensure_supported(ps)?;
    let (usable_w, usable_h) = usable_page_pt(ps);
    if usable_w <= 0.0 || usable_h <= 0.0 {
        return Err(EngineError::UnsupportedPaging {
            detail: format!(
                "可用页尺寸非正（{usable_w:.2}×{usable_h:.2}pt）：边距 ≥ 纸张（paper={}）",
                ps.paper
            ),
        });
    }
    let mut pg = paginate(
        grid,
        usable_w as f32,
        usable_h as f32,
        ps.table_column_num as i32,
    );
    pg.row_num_per_page = ps.row_num_per_page;
    Ok(pg)
}

#[cfg(test)]
mod tests;
