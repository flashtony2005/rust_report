//! 按分页器给出的实际行列清单生成页快照。
use rpx_core::ReportDefine;
use rpx_engine::{
    arena::CalcGrid,
    pager::{project_page, Pagination},
};
use serde_json::{json, Map, Value as J};

pub(crate) fn build_paged(
    rd: &ReportDefine,
    grid: &CalcGrid,
    pg: &Pagination,
    name: &str,
) -> Result<String, String> {
    let mut sheets = Map::new();
    let mut order = Vec::new();
    for page in &pg.pages {
        let projected = project_page(grid, page);
        let text = super::build_single(rd, &projected, name)?;
        let mut workbook: J = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let mut sheet = workbook["sheets"]["rpx-sheet-1"].take();
        let id = format!("rpx-sheet-{}", page.page);
        sheet["id"] = json!(id);
        sheet["name"] = json!(format!("第 {} 页", page.page));
        order.push(id.clone());
        sheets.insert(id, sheet);
    }
    serde_json::to_string(&json!({
        "id":format!("rpx-{name}"),"name":name,"appVersion":"1.0.0","locale":"zhCN",
        "styles":{},"sheetOrder":order,"sheets":sheets,"pagination":super::pagination_json(grid)
    }))
    .map_err(|e| e.to_string())
}
