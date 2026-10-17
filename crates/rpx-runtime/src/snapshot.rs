//! 快照映射：`CalcGrid`（引擎计算态）→ Univer `IWorkbookData` JSON。
//!
//! 契约（字段表 / 映射表 / `mergeData` 去重算法）：`docs/spec-m4.md` **§4** ——
//! 本模块只作实现，不复制那张表；改契约先改 §4。
//!
//! # 输入与出处
//!
//! * `rd`：模板 —— 样式来源：格经 `CalcCell.src`（模板出处坐标）回查 `rd.cell_at`；
//! * `grid`：`rpx_engine::calculate` 的产物（值 / 合并 / 行列尺寸）。
//!
//! # 声明近似（不静默）
//!
//! * 单位换算 mm→px 用 96dpi 线性式，**无产品基准可对拍**（`spec-m4.md` §7）；
//! * `Decimal` → number 丢尾零（显示对齐优先；数字格式 `n` 后置补，同 §7）。
//!
//! # 分页口径
//!
//! v1 **单 sheet 全量行**（不做页拆分）：`grid.merged` 的 `r/c` 是计算矩阵坐标、
//! 与页拆分无关（`arena/calc_grid.rs` 的 `MergedCell` 文档：槽枚举发生在合并趟、
//! 早于分页）；多 sheet 待 M3-06 真实口径后接线（`spec-m4.md` §5 M4-05）。

use serde_json::{json, Map, Value as J};

use rpx_core::ReportDefine;
use rpx_engine::arena::CalcGrid;

mod freeze;
pub(crate) mod geom;
pub(crate) mod graphics;
mod paged;
pub(crate) mod style;
pub(crate) mod value;

#[cfg(test)]
mod tests;

/// 生成快照 JSON 文本（`IWorkbookData`；紧凑输出，调试用 `python -m json.tool`）。
///
/// `name` 是报表名（`main.rs` 取 `.rpx` 文件主干传入）。
///
/// `Err` = 映射不变式被违反（当前唯一来源：[`geom::merge_data`] 的网格校验）
/// —— 按「诚实失败优于静默错」直接上抛，不产出半成品快照。
///
/// # 分页分派（M4-05）
///
/// `grid.pagination`（`/render` 已覆盖为**模板真值口径**）页数 > 1 ⇒
/// **多 sheet 切片**（每页一个 sheet，坐标平移，[`paged`] 模块）；
/// 单页 / 手搭网格（无 pagination）⇒ 单 sheet 全量（本函数余部 + [`pagination_json`]）。
/// 夹具对拍走 `rpx_engine` 的 emit（不经本模块）⇒ 结构变更不炸对拍。
pub fn build(rd: &ReportDefine, grid: &CalcGrid, name: &str) -> Result<String, String> {
    if let Some(pg) = &grid.pagination {
        if pg.page_count > 1 {
            return paged::build_paged(rd, grid, pg, name);
        }
        if let Some(page) = pg.pages.first() {
            let mut projected = rpx_engine::pager::project_page(grid, page);
            projected.pagination = grid.pagination.clone();
            projected.page_count = grid.page_count;
            return build_single(rd, &projected, name);
        }
    }
    build_single(rd, grid, name)
}

/// 单 sheet 路径（M3/M4 v1 契约，多页报表不走这里）。
fn build_single(rd: &ReportDefine, grid: &CalcGrid, name: &str) -> Result<String, String> {
    let merge_data = geom::merge_data(grid)?;
    let graphics = graphics::for_grid(grid)?;
    let sheet_id = "rpx-sheet-1";
    let sheet = json!({
        "id": sheet_id,
        "name": "Sheet1",
        "rowCount": (grid.rows + 5).max(100),
        "columnCount": (grid.cols + 3).max(26),
        "defaultRowHeight": geom::default_row_height_px(),
        "defaultColumnWidth": geom::default_col_width_px(),
        "showGridlines": 1,
        "freeze": freeze::for_grid(grid),
        "cellData": cell_data(rd, grid),
        "mergeData": merge_data,
        "rowData": geom::row_data(grid),
        "columnData": geom::col_data(grid),
        "rpxGraphics":graphics,
        "pageBackground":grid.page_background,
        "printOptions":{
            "autoSize":grid.print_options.auto_size,"wrapNumbers":grid.print_options.wrap_numbers,
            "borderWidth":grid.print_options.border_width,"borderColor":format!("#{:06X}",grid.print_options.border_color),
            "overlay":grid.print_setup.back_graph_printed
        },
        "printRowCount":grid.rows,"printColumnCount":grid.cols,
        "printSize":{
            "width":rpx_engine::pager::paper_width_mm(grid.print_setup.paper,grid.print_setup.paper_width),
            "height":rpx_engine::pager::paper_height_mm(grid.print_setup.paper,grid.print_setup.paper_height),
            "margins":[grid.print_setup.top_margin,grid.print_setup.right_margin,grid.print_setup.bottom_margin,grid.print_setup.left_margin]
        },
    });
    let wb = json!({
        "id": format!("rpx-{name}"),
        "name": name,
        "appVersion": "1.0.0",
        "locale": "zhCN",
        "styles": {},
        "sheetOrder": [sheet_id],
        "sheets": { sheet_id: sheet },
        // M3-06：`pagination` 是 **rpx 扩展键**（非 Univer `IWorkbookData` 字段，
        // 前端/Univer 忽略之）：`/render` 的模板真值口径分页观测，
        // M4-05（翻页）的输入。契约见 `docs/spec-m4.md` §4.5。
        "pagination": pagination_json(grid),
    });
    Ok(serde_json::to_string(&wb)
        .expect("快照 JSON 序列化失败：value/geom 已过滤非有限数，不应发生"))
}

/// `pagination` 块（M3-06，契约 `docs/spec-m4.md` §4.5）：workbook 顶层。
///
/// `grid.pagination` 是 `render_request` 里刚覆盖过的**模板真值口径**
/// （纸码表 + 边距；`http.rs` 接线）——不是探针口径。`pages` 的
/// `row_range`/`col_range` 由引擎 1 起坐标转 **0 起**（与 §4.2 同规）；
/// ⚠ 页首区带/分组头副本每页重复 ⇒ 相邻页的 `row_range` **可以重叠**（如实）。
/// `grid.pagination` 为 `None`（手搭网格，HTTP 路径不会发生）⇒ 写 `null`。
pub(crate) fn pagination_json(grid: &CalcGrid) -> J {
    match &grid.pagination {
        None => J::Null,
        Some(pg) => json!({
            "page_w_pt": pg.paper_w,
            "page_h_pt": pg.paper_h,
            "columns": pg.columns,
            "page_count": pg.page_count,
            "x_pages": pg.x_pages,
            "y_pages": pg.y_pages,
            "row_num_per_page": pg.row_num_per_page,
            "pages": pg
                .pages
                .iter()
                .map(|p| {
                    json!({
                        "page": p.page,
                        "rows": p.rows,
                        "row_range": [
                            p.row_range.0.saturating_sub(1),
                            p.row_range.1.saturating_sub(1)
                        ],
                        "col_range": [
                            p.col_range.0.saturating_sub(1),
                            p.col_range.1.saturating_sub(1)
                        ],
                    })
                })
                .collect::<Vec<_>>(),
        }),
    }
}

/// `cellData`：`{"r": {"c": ICellData}}`，坐标 **1 起 → 0 起**（`spec-m4.md` §4.2）。
///
/// 过滤 `CalcCell::is_emitted`（值或显示值至少一个非空）——与夹具 `cells` 的
/// 稀疏口径一致（`arena/calc_grid.rs` 的 `is_emitted` 文档）。
fn cell_data(rd: &ReportDefine, grid: &CalcGrid) -> J {
    let mut rows: Map<String, J> = Map::new();
    for cell in &grid.cells {
        if !cell.is_emitted() {
            continue;
        }
        let mut one = Map::new();
        // 显示值优先（`spec-m4.md` §4.3.1 末行）：它是产品的"最终显示"覆盖项，
        // 存在时胜过原值 —— `engine_disp` 夹具 (2,3) 即 `value=null` + `disp_value`。
        match &cell.disp_value {
            Some(d) => {
                one.insert("v".to_string(), J::String(d.clone()));
                one.insert("t".to_string(), json!(value::T_FORCE_STRING));
            }
            None => {
                let (v, t) = value::value_json(&cell.value);
                one.insert("v".to_string(), v);
                if let Some(t) = t {
                    one.insert("t".to_string(), json!(t));
                }
            }
        }
        // 样式：经模板出处回查模板格；无 `src` 的格（引擎生成的占位）不出样式。
        if let Some(tc) = rd.cell_at(cell.origin.0 as usize, cell.origin.1 as usize) {
            one.insert("s".to_string(), style::style_json(tc));
        }
        let row = rows
            .entry((cell.row - 1).to_string())
            .or_insert_with(|| J::Object(Map::new()));
        if let Some(m) = row.as_object_mut() {
            m.insert((cell.col - 1).to_string(), J::Object(one));
        }
    }
    J::Object(rows)
}
