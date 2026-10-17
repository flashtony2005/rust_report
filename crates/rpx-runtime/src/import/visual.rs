use super::*;
use std::collections::BTreeMap;
pub(super) fn apply(
    j: &mut J,
    r: usize,
    c: usize,
    values: &BTreeMap<String, String>,
) -> Result<()> {
    let get = |k: &str| {
        values
            .get(k)
            .map(String::as_str)
            .ok_or_else(|| unsupported(&format!("/cells/{r}/{c}/graphic/{k}")))
    };
    let kind = get("type")?;
    let cell = j["cells"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|v| v["row"] == r && v["col"] == c)
        .ok_or_else(|| unsupported("/graphics/cell"))?;
    let number =
        |k: &str| -> Result<J> { serde_json::from_str(get(k)?).map_err(|_| unsupported(k)) };
    let mut graphic = json!({"width":number("width")?,"height":number("height")?});
    let key = match kind {
        "image" | "cell_background" => {
            graphic["source"] = json!(get("source")?);
            "image"
        }
        "echarts" | "echarts_js" => {
            graphic["template"] = json!(get("template")?);
            let mut params = json!({});
            for (k, v) in values {
                if let Some(k) = k.strip_prefix("param.") {
                    params[k] = json!(v);
                }
            }
            graphic["params"] = params;
            "echarts"
        }
        _ => {
            graphic["type"] = json!(kind);
            graphic["title"] = json!(get("title")?);
            if let Some(n) = values.get("series_count") {
                let count = n
                    .parse::<usize>()
                    .map_err(|_| unsupported("/graphics/series_count"))?;
                if count > 256 {
                    return Err(unsupported("/graphics/series_count"));
                }
                let mut series = Vec::new();
                for i in 0..count {
                    series.push(json!({"name":get(&format!("series.{i}.name"))?,"values":get(&format!("series.{i}.values"))?}));
                }
                graphic["series"] = json!(series);
                graphic["categories"] = json!(get("categories")?);
            } else {
                let mut tasks = json!({});
                for k in ["categories", "starts", "ends", "states"] {
                    if let Some(v) = values.get(&format!("tasks.{k}")) {
                        tasks[k] = json!(v);
                    }
                }
                graphic["tasks"] = tasks;
            }
            let mut appearance = json!({});
            for k in ["legend", "background"] {
                if let Some(v) = values.get(k) {
                    appearance[k] = json!(v);
                }
            }
            for k in ["pie_rotation", "pie_depth"] {
                if values.contains_key(k) {
                    appearance[k] = number(k)?;
                }
            }
            if let Some(v) = values.get("colors") {
                appearance["colors"] = json!(v.split(',').collect::<Vec<_>>());
            }
            if values.contains_key("margin.top") {
                appearance["margins"] = json!([
                    number("margin.top")?,
                    number("margin.right")?,
                    number("margin.bottom")?,
                    number("margin.left")?
                ]);
            }
            graphic["appearance"] = appearance;
            "chart"
        }
    };
    cell["data_type"] = json!(if key == "chart" { "chart" } else { kind });
    cell[key] = graphic;
    Ok(())
}
