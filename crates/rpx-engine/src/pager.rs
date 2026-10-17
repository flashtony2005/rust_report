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
//! # 两条明确的计算口径
//!
//! `paginate` 是固定纸张的探针观测；`paginate_with_setup` 使用模板设置，
//! 支持不分页、按行、按纸、区带、强制分页、多栏与打印矩阵。
//! 服务面快照与打印 XLSX 共享 `project_page`，页副本处理合并、显示值与页末空白。
//! 文本尺寸在浏览器测量；尚未对 Java 的字体布局逐像素对拍。
//!
mod arith;
mod blocks;
mod compose;
mod configured;
mod finishing;
mod guard;
pub mod options;
mod paper;
mod project;
mod split;
pub mod style;
pub use project::project_page;

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
pub use rpx_core::print_setup::PrintSetup;

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
    /// 分栏页的各栏。单栏页为空；子栏始终是普通行列投影。
    pub panels: Vec<PageInfo>,
    /// 页内行序对应的全局行号，包含重复表头。
    pub row_indices: Vec<i32>,
    /// 页内列序对应的全局列号，包含左右表头。
    pub col_indices: Vec<i32>,
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
/// `columns` 在探针口径中只作回显；模板配置通过 `paginate_with_setup` 处理。
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
                panels: Vec::new(),
                row_indices: page_rows.iter().map(|r| r.row).collect(),
                col_indices: (c0..=c1).collect(),
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
/// 独立的模板配置算法，保留与探针观测不同的纸张、区带与分栏契约。
/// 复核分页配置与可用尺寸；边距占满纸张时返回明确错误。
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
    configured::paginate(grid, ps, usable_w, usable_h)
}

#[cfg(test)]
mod tests;
