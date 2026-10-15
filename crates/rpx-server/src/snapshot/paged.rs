//! M4-05 多 sheet 翻页：按 `pagination.pages` 把全量网格**切片**成每页一个
//! sheet（契约 `docs/spec-m4.md` §4.6）。
//!
//! # 切片语义（与产品 `SimplePager` 输出的对应）
//!
//! 产品分页器每页产出**独立格矩阵**（含页首区带/分组头副本）；本模块从同一份
//! 全量计算网格按页 `row_range`/`col_range`（0 起闭区间，来自 M3-06 真值口径）
//! 切片——页间重叠的区带副本因此**自然每页重现**，与产品每页独立矩阵同观感。
//!
//! # 跨页合并拆分（产品语义已实现，2026-10-15 源码实锤）
//!
//! 产品对跨页合并做**拆分**（延续段独立成块）：`SimplePager._$1` 行向在断行处
//! 深克隆锚点、上页块裁到页末、下页块从断行起新 Area（`SimplePager.java:1624-1638`
//! ——`deepClone` + `newArrea` + `ar.setEndRow(_$28-1)`）；列向同构（`:2316-2335`
//! ——`deepClone` + `new Area(startRow, col, endRow, endCol)` + `ar.setEndCol(col-1)`）。
//! 本模块按**页矩形 × 全局块求交**同构实现：交叠即块；锚点在页外时在交叠区
//! 左上角注入**克隆锚点**（值/样式取原锚点格——产品 `deepClone` 的快照等价物）。
//!
//! ⚠ 仍不建模的退化分支（与换行回填同账，`spec-m3.md` §5）：文本高于整页时
//! 产品**整块搬移**而非拆分（`:1574-1622`，需字体测量）；标题行 `rt == -90`
//! 同走整块搬移。夹具无此场景，按"禁止无基准对拍"留账。
//!
//! # 坐标
//!
//! 页内坐标 = 全局坐标 − 页偏移（0 起键/值，与 §4.2 同规）。

use serde_json::{json, Map, Value as J};

use rpx_core::ReportDefine;
use rpx_engine::arena::CalcGrid;
use rpx_engine::pager::Pagination;

use super::{geom, style, value};

/// 多 sheet 快照：每页一个 `rpx-sheet-{n}`（n = 页码，1 起），`sheetOrder`
/// 顺序即页序——Univer 底部 tab 即「翻页」。顶层 `pagination` 块保留
/// （§4.5 观测口径不变）。
pub(crate) fn build_paged(
    rd: &ReportDefine,
    grid: &CalcGrid,
    pg: &Pagination,
    name: &str,
) -> Result<String, String> {
    let merges = super::geom::merge_data(grid)?;
    let mut sheets = Map::new();
    let mut order = Vec::new();
    for p in &pg.pages {
        // 页区间转 0 起闭区间（与 §4.5 的 `row_range`/`col_range` 同源同规）。
        let (r0, r1) = (
            p.row_range.0.saturating_sub(1),
            p.row_range.1.saturating_sub(1),
        );
        let (c0, c1) = (
            p.col_range.0.saturating_sub(1),
            p.col_range.1.saturating_sub(1),
        );
        // 合并块 × 页矩形求交 + 延续克隆锚点收集（模块文档「跨页合并拆分」）。
        let split = merge_data_paged(&merges, r0, r1, c0, c1);
        let sheet_id = format!("rpx-sheet-{}", p.page);
        order.push(J::String(sheet_id.clone()));
        let sheet = json!({
            "id": sheet_id,
            "name": format!("第 {} 页", p.page),
            "rowCount": (r1 - r0 + 1 + 5).max(100),
            "columnCount": (c1 - c0 + 1 + 3).max(26),
            "defaultRowHeight": geom::default_row_height_px(),
            "defaultColumnWidth": geom::default_col_width_px(),
            "showGridlines": 1,
            "cellData": cell_data_paged(rd, grid, r0, r1, c0, c1, &split.clones),
            "mergeData": split.blocks,
            "rowData": row_data_paged(grid, r0, r1),
            "columnData": col_data_paged(grid, c0, c1),
        });
        sheets.insert(sheet_id, sheet);
    }
    let wb = json!({
        "id": format!("rpx-{name}"),
        "name": name,
        "appVersion": "1.0.0",
        "locale": "zhCN",
        "styles": {},
        "sheetOrder": order,
        "sheets": J::Object(sheets),
        "pagination": super::pagination_json(grid),
    });
    Ok(serde_json::to_string(&wb)
        .expect("快照 JSON 序列化失败：value/geom 已过滤非有限数，不应发生"))
}

/// 页内 `cellData`：过滤 + 平移（1 起计算态 → 页内 0 起）。
///
/// `clones` = 延续克隆锚点（页内 0 起 (行, 列), 全局 0 起锚点 (行, 列)）：
/// 在正格铺完后注入——与产品 `setCell(r, j, c2)` 的**覆盖**语义一致
/// （`SimplePager.java:1632-1634`）。锚点格缺值（未 emit）时注入空格：块几何
/// 仍在（mergeData），显示为空——与产品"克隆空锚"同观感。
fn cell_data_paged(
    rd: &ReportDefine,
    grid: &CalcGrid,
    r0: i32,
    r1: i32,
    c0: i32,
    c1: i32,
    clones: &[(i32, i32, i32, i32)],
) -> J {
    let mut rows: Map<String, J> = Map::new();
    for cell in &grid.cells {
        if !cell.is_emitted() {
            continue;
        }
        let (gr, gc) = (cell.row - 1, cell.col - 1); // 全局 0 起
        if gr < r0 || gr > r1 || gc < c0 || gc > c1 {
            continue;
        }
        let one = cell_json(rd, cell);
        let row = rows
            .entry((gr - r0).to_string())
            .or_insert_with(|| J::Object(Map::new()));
        if let Some(m) = row.as_object_mut() {
            m.insert((gc - c0).to_string(), one);
        }
    }
    // 延续克隆锚点注入（后于正格 ⇒ 同位覆盖，同产品）。
    for (pr, pc, sr, sc) in clones {
        let one = find_cell(grid, *sr, *sc)
            .filter(|c| c.is_emitted())
            .map(|c| cell_json(rd, c))
            .unwrap_or_else(|| J::Object(Map::new()));
        let row = rows
            .entry(pr.to_string())
            .or_insert_with(|| J::Object(Map::new()));
        if let Some(m) = row.as_object_mut() {
            m.insert(pc.to_string(), one);
        }
    }
    J::Object(rows)
}

/// 单格 JSON（显示值/值 + 模板样式）——正格与延续克隆锚点共用同一份口径。
fn cell_json(rd: &ReportDefine, cell: &rpx_engine::arena::CalcCell) -> J {
    let mut one = Map::new();
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
    if let Some(src) = &cell.src {
        if let Some(tc) = rd.cell_at(src.src_row as usize, src.src_col as usize) {
            one.insert("s".to_string(), style::style_json(tc));
        }
    }
    J::Object(one)
}

/// 全局 0 起 `(gr0, gc0)` 找计算格。
fn find_cell(grid: &CalcGrid, gr0: i32, gc0: i32) -> Option<&rpx_engine::arena::CalcCell> {
    grid.cells
        .iter()
        .find(|c| c.row == gr0 + 1 && c.col == gc0 + 1)
}

/// 每页合并块切片：全局块 × 页矩形求交。
pub(crate) struct PageMergeSplit {
    /// 该页 `mergeData`（页内 0 起坐标）。
    blocks: Vec<J>,
    /// 延续克隆锚点：(页内 0 起行, 页内 0 起列, 全局 0 起锚点行, 全局 0 起锚点列)。
    clones: Vec<(i32, i32, i32, i32)>,
}

/// 页内 `mergeData`：块 × 页矩形求交平移；锚点落在页外（延续页）时在交叠区
/// 左上角收集克隆锚点（模块文档「跨页合并拆分」，产品 `SimplePager.java:1624-1638`
/// 行向 / `:2316-2335` 列向）。合并区只会向右下延伸——交叠为空即该页无此块。
fn merge_data_paged(merges: &[J], r0: i32, r1: i32, c0: i32, c1: i32) -> PageMergeSplit {
    let mut blocks = Vec::new();
    let mut clones = Vec::new();
    for m in merges {
        let (Some(sr), Some(sc), Some(er), Some(ec)) = (
            m["startRow"].as_i64(),
            m["startColumn"].as_i64(),
            m["endRow"].as_i64(),
            m["endColumn"].as_i64(),
        ) else {
            continue;
        };
        let (sr, sc, er, ec) = (sr as i32, sc as i32, er as i32, ec as i32);
        let (ir0, ir1) = (sr.max(r0), er.min(r1));
        let (ic0, ic1) = (sc.max(c0), ec.min(c1));
        if ir0 > ir1 || ic0 > ic1 {
            continue;
        }
        blocks.push(json!({
            "startRow": ir0 - r0,
            "startColumn": ic0 - c0,
            "endRow": ir1 - r0,
            "endColumn": ic1 - c0,
        }));
        if sr < r0 || sc < c0 {
            clones.push((ir0 - r0, ic0 - c0, sr, sc));
        }
    }
    PageMergeSplit { blocks, clones }
}

/// 页内 `rowData`：键 = 全局行号 − r0（行高来自全量数组切片）。
fn row_data_paged(grid: &CalcGrid, r0: i32, r1: i32) -> J {
    let mut m = Map::new();
    for (i, h) in grid.row_heights.iter().enumerate() {
        let gi = i as i32;
        if gi >= r0 && gi <= r1 {
            m.insert(
                (gi - r0).to_string(),
                json!({ "h": geom::size_px(*h, grid.unit) }),
            );
        }
    }
    J::Object(m)
}

/// 页内 `columnData`：同 [`row_data_paged`]。
fn col_data_paged(grid: &CalcGrid, c0: i32, c1: i32) -> J {
    let mut m = Map::new();
    for (i, w) in grid.col_widths.iter().enumerate() {
        let gi = i as i32;
        if gi >= c0 && gi <= c1 {
            m.insert(
                (gi - c0).to_string(),
                json!({ "w": geom::size_px(*w, grid.unit) }),
            );
        }
    }
    J::Object(m)
}
