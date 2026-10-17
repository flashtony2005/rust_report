use rpx_engine::{
    arena::CalcGrid,
    graphics::{GraphicContent, Series},
};
use serde_json::{json, Value as J};

pub(crate) fn validate(grid: &CalcGrid) -> Result<(), String> {
    if let Some(uri) = &grid.page_background {
        rpx_export::validate_image(uri)?;
    }
    for g in &grid.graphics {
        match &g.content {
            GraphicContent::Image(uri) | GraphicContent::CellBackground(uri) => {
                rpx_export::validate_image(uri)?
            }
            GraphicContent::Echarts(option) => {
                let option: J = serde_json::from_str(option).map_err(|e| {
                    format!("图形 ({},{}) ECharts option JSON 无效：{e}", g.row, g.col)
                })?;
                if !option.is_object() {
                    return Err(format!(
                        "图形 ({},{}) ECharts option 必须为对象",
                        g.row, g.col
                    ));
                }
            }
            GraphicContent::Chart(_) | GraphicContent::EchartsScript(_) => {}
        }
    }
    Ok(())
}

pub(super) fn for_grid(grid: &CalcGrid) -> Result<J, String> {
    let mut out = Vec::new();
    for g in &grid.graphics {
        let content = match &g.content {
            GraphicContent::Chart(c) => json!({"kind":"chart","type":c.kind.name(),"title":c.title,
                "appearance":{"colors":c.appearance.colors,"legend":c.appearance.legend,"background":c.appearance.background,"margins":c.appearance.margins,"pieRotation":c.appearance.pie_rotation,"pieDepth":c.appearance.pie_depth},
                "tasks":c.tasks.as_ref().map(|t|json!({"categories":t.categories.iter().map(value).collect::<Vec<_>>(),"starts":t.starts.iter().map(value).collect::<Vec<_>>(),"ends":t.ends.iter().map(value).collect::<Vec<_>>(),"states":t.states.iter().map(value).collect::<Vec<_>>()})),
                "categories":c.categories.iter().map(value).collect::<Vec<_>>(),
                "series":c.series.iter().map(series).collect::<Vec<_>>() }),
            GraphicContent::Image(uri) => json!({"kind":"image","source":uri}),
            GraphicContent::CellBackground(uri) => json!({"kind":"cell_background","source":uri}),
            GraphicContent::Echarts(option) => {
                json!({"kind":"echarts","option":serde_json::from_str::<J>(option).map_err(|e|format!("图形 ({},{}) ECharts 模板生成的 option JSON 无效：{e}",g.row,g.col))?})
            }
            GraphicContent::EchartsScript(source) => json!({"kind":"echarts_js","source":source}),
        };
        out.push(json!({"row":g.row-1,"col":g.col-1,"width":g.width,"height":g.height,"content":content}));
    }
    Ok(J::Array(out))
}
fn value(v: &rpx_engine::value::Value) -> J {
    super::value::value_json(v).0
}
fn series(s: &Series) -> J {
    json!({"name":s.name,"values":s.values.iter().map(value).collect::<Vec<_>>()})
}
