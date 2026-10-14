//! M4-05 多 sheet 翻页：按 `pagination.pages` 把全量网格**切片**成每页一个
//! sheet（契约 `docs/spec-m4.md` §4.6）。
//!
//! # 切片语义（与产品 `SimplePager` 输出的对应）
//!
//! 产品分页器每页产出**独立格矩阵**（含页首区带/分组头副本）；本模块从同一份
//! 全量计算网格按页 `row_range`/`col_range`（0 起闭区间，来自 M3-06 真值口径）
//! 切片——页间重叠的区带副本因此**自然每页重现**，与产品每页独立矩阵同观感。
//!
//! # 声明近似（不静默，`spec-m4.md` §4.6 末行）
//!
//! **跨页合并区裁剪**：锚点落在页内的合并区，跨度裁到页边界（显示为截断的
//! 合并块）；产品对跨页合并做**拆分**（延续段独立成块）——两者的显示差异
//! 仅在块边缘是否存在延续锚点。产品拆分语义无夹具基准（`merged.cells` 全空
//! 不可辨，M2 带账）⇒ 按纪律不冒充等价，spec 记账。
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
            "cellData": cell_data_paged(rd, grid, r0, r1, c0, c1),
            "mergeData": merge_data_paged(&merges, r0, r1, c0, c1),
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
fn cell_data_paged(rd: &ReportDefine, grid: &CalcGrid, r0: i32, r1: i32, c0: i32, c1: i32) -> J {
    let mut rows: Map<String, J> = Map::new();
    for cell in &grid.cells {
        if !cell.is_emitted() {
            continue;
        }
        let (gr, gc) = (cell.row - 1, cell.col - 1); // 全局 0 起
        if gr < r0 || gr > r1 || gc < c0 || gc > c1 {
            continue;
        }
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
        if let Some(src) = cell.src {
            if let Some(tc) = rd.cell_at(src.src_row as usize, src.src_col as usize) {
                one.insert("s".to_string(), style::style_json(tc));
            }
        }
        let row = rows
            .entry((gr - r0).to_string())
            .or_insert_with(|| J::Object(Map::new()));
        if let Some(m) = row.as_object_mut() {
            m.insert((gc - c0).to_string(), J::Object(one));
        }
    }
    J::Object(rows)
}

/// 页内 `mergeData`：含于页内的直接平移；**跨页合并区裁剪**到页界
/// （锚点在页内才保留——声明近似见模块文档）。
fn merge_data_paged(merges: &[J], r0: i32, r1: i32, c0: i32, c1: i32) -> Vec<J> {
    merges
        .iter()
        .filter_map(|m| {
            let sr = m["startRow"].as_i64()? as i32;
            let sc = m["startColumn"].as_i64()? as i32;
            let er = m["endRow"].as_i64()? as i32;
            let ec = m["endColumn"].as_i64()? as i32;
            if sr < r0 || sc < c0 || sr > r1 || sc > c1 {
                return None; // 锚点在页外：整块归页外页
            }
            Some(json!({
                "startRow": sr - r0,
                "startColumn": sc - c0,
                "endRow": er.min(r1) - r0,
                "endColumn": ec.min(c1) - c0,
            }))
        })
        .collect()
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
