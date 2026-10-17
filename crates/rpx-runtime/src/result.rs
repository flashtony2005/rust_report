use crate::{
    definition::{address, coord},
    error::{Error, Result},
    files,
};
use rpx_engine::{arena::CalcGrid, Value};
use serde_json::{json, Value as J};

pub fn typed(v: &Value) -> J {
    let value = match v {
        Value::Null => J::Null,
        Value::Bool(v) => json!(v),
        Value::Byte(v) => json!(v),
        Value::Short(v) => json!(v),
        Value::Int(v) => json!(v),
        Value::Long(v) => json!(v.to_string()),
        Value::Float(v) => json!(v),
        Value::Double(v) => json!(v),
        Value::Decimal(v) | Value::Text(v) => json!(v),
        Value::List(v) => json!(v.iter().map(typed).collect::<Vec<_>>()),
    };
    json!({"type":v.type_name(),"value":value})
}
pub fn build(g: &CalcGrid, source: &J) -> J {
    let mut cells:Vec<_>=g.cells.iter().map(|c| {
        let mut v=json!({"row":c.row,"col":c.col,"address":address(c.row as u64,c.col as u64),
            "origin":address(c.origin.0 as u64,c.origin.1 as u64),"origin_coordinates":[c.origin.0,c.origin.1],
            "value":typed(&c.value),"left_head_origin":c.left_head.map(|(r,c)|address(r as u64,c as u64)),
            "top_head_origin":c.top_head.map(|(r,c)|address(r as u64,c as u64))});
        if let Some(d)=&c.disp_value {v["display_override"]=json!(d);}
        v
    }).collect();
    cells.sort_by_key(|c| (c["row"].as_i64(), c["col"].as_i64()));
    let merges:Vec<_>=g.merged.iter().map(|m|json!({"row":m.r,"col":m.c,"rows":m.row_merge,"cols":m.col_merge,"value":typed(&m.value)})).collect();
    json!({"version":1,"rows":g.rows,"cols":g.cols,"cells":cells,"merges":merges,
        "pagination":crate::snapshot::pagination_json(g),"template":{"grid":source["grid"]},
        "row_heights":g.row_heights,"col_widths":g.col_widths})
}
pub fn inspect(
    j: &J,
    range: Option<&str>,
    origin: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<J> {
    let bounds = range
        .map(|r| {
            let (a, b) = r.split_once(':').unwrap_or((r, r));
            let a = coord(a)?;
            let b = coord(b)?;
            if a.0 > b.0 || a.1 > b.1 {
                return Err(Error::new(
                    2,
                    "USAGE",
                    "arguments",
                    "范围终点必须在起点之后",
                ));
            }
            Ok((a, b))
        })
        .transpose()?;
    let origin = origin.map(coord).transpose()?.map(|(r, c)| address(r, c));
    if j.get("version").and_then(J::as_u64) != Some(1) {
        return Err(Error::new(
            3,
            "RESULT_INVALID",
            "result",
            "结果缺少 version=1（这可能是模板定义，不是运行结果）",
        ));
    }
    let cells = j["cells"]
        .as_array()
        .ok_or_else(|| Error::new(3, "RESULT_INVALID", "result", "结果没有 cells 数组"))?;
    if j.get("rows").and_then(J::as_u64).is_none() || j.get("cols").and_then(J::as_u64).is_none() {
        return Err(Error::new(
            3,
            "RESULT_INVALID",
            "result",
            "结果缺少 rows/cols",
        ));
    }
    if let Some((i, _)) = cells
        .iter()
        .enumerate()
        .find(|(_, c)| !c.get("value").is_some_and(J::is_object))
    {
        return Err(
            Error::new(3, "RESULT_INVALID", "result", "结果 cell 缺少类型化 value")
                .at(format!("/cells/{i}/value")),
        );
    }
    let selected: Vec<_> = cells
        .iter()
        .filter(|c| {
            let r = c["row"].as_u64().unwrap_or(0);
            let col = c["col"].as_u64().unwrap_or(0);
            bounds.is_none_or(|(a, b)| r >= a.0 && r <= b.0 && col >= a.1 && col <= b.1)
                && origin
                    .as_ref()
                    .is_none_or(|o| c["origin"].as_str() == Some(o))
        })
        .cloned()
        .collect();
    let mut page = files::page(&selected, limit, offset)?;
    page["shape"] = json!({"rows":j["rows"],"cols":j["cols"]});
    Ok(page)
}
