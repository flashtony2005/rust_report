use super::{
    graphics::{expression, unknown},
    json::{arr, as_str, as_u64, err, field, obj, opt_str},
    DefError,
};
use rpx_engine::graphics::ChartKind;
use serde_json::{Map, Value as J};
pub(super) fn apply(
    o: &Map<String, J>,
    path: &str,
    put: &mut impl FnMut(&str, String),
) -> Result<(), DefError> {
    unknown(
        o,
        &[
            "type",
            "title",
            "categories",
            "series",
            "tasks",
            "appearance",
            "width",
            "height",
        ],
        path,
    )?;
    let name = as_str(field(o, "type", path)?, &format!("{path}.type"))?;
    let kind = ChartKind::parse(name)
        .ok_or_else(|| err(&format!("{path}.type"), format!("未实现的统计图 {name:?}")))?;
    put("type", name.into());
    put("title", opt_str(o, "title", path)?.unwrap_or("").into());
    if kind.intervals() {
        if o.contains_key("categories") || o.contains_key("series") {
            return Err(err(path, "任务图使用 tasks，不使用分类轴/系列"));
        }
        let p = format!("{path}.tasks");
        let tasks = obj(field(o, "tasks", path)?, &p)?;
        unknown(tasks, &["categories", "starts", "ends", "states"], &p)?;
        for key in ["categories", "starts", "ends", "states"] {
            if let Some(v) = tasks.get(key) {
                put(
                    &format!("tasks.{key}"),
                    expression(v, &format!("{p}.{key}"))?,
                )
            } else if ["categories", "starts"].contains(&key)
                || (key == "ends" && kind != ChartKind::Milestone)
                || (key == "states" && kind == ChartKind::StateTimeline)
            {
                return Err(err(&format!("{p}.{key}"), "缺少任务绑定"));
            }
        }
    } else {
        if o.contains_key("tasks") {
            return Err(err(&format!("{path}.tasks"), "该图使用 categories/series"));
        }
        put(
            "categories",
            expression(field(o, "categories", path)?, &format!("{path}.categories"))?,
        );
        let series = arr(field(o, "series", path)?, &format!("{path}.series"))?;
        if series.is_empty() || series.len() > 256 {
            return Err(err(&format!("{path}.series"), "需要 1..=256 个系列"));
        }
        put("series_count", series.len().to_string());
        for (i, s) in series.iter().enumerate() {
            let p = format!("{path}.series[{i}]");
            let s = obj(s, &p)?;
            unknown(s, &["name", "values"], &p)?;
            put(
                &format!("series.{i}.name"),
                as_str(field(s, "name", &p)?, &format!("{p}.name"))?.into(),
            );
            put(
                &format!("series.{i}.values"),
                expression(field(s, "values", &p)?, &format!("{p}.values"))?,
            );
        }
    }
    if let Some(a) = o.get("appearance") {
        appearance(
            obj(a, &format!("{path}.appearance"))?,
            &format!("{path}.appearance"),
            put,
        )?
    }
    Ok(())
}
fn color(s: &str) -> bool {
    s == "transparent"
        || (s.len() == 7 && s.starts_with('#') && u32::from_str_radix(&s[1..], 16).is_ok())
}
fn appearance(
    o: &Map<String, J>,
    path: &str,
    put: &mut impl FnMut(&str, String),
) -> Result<(), DefError> {
    unknown(
        o,
        &[
            "colors",
            "legend",
            "background",
            "margins",
            "pie_rotation",
            "pie_depth",
        ],
        path,
    )?;
    for (k, v) in o {
        let p = format!("{path}.{k}");
        match k.as_str() {
            "colors" => {
                let colors = arr(v, &p)?
                    .iter()
                    .map(|v| as_str(v, &p))
                    .collect::<Result<Vec<_>, _>>()?;
                if colors.is_empty() || colors.iter().any(|s| *s == "transparent" || !color(s)) {
                    return Err(err(&p, "需要非空的 #RRGGBB 配色列表"));
                }
                put(k, colors.join(","))
            }
            "background" => {
                let s = as_str(v, &p)?;
                if !color(s) {
                    return Err(err(&p, "需要 #RRGGBB 或 transparent"));
                }
                put(k, s.into())
            }
            "legend" => {
                let s = as_str(v, &p)?;
                if !["top", "right", "bottom", "left", "none"].contains(&s) {
                    return Err(err(&p, "可选 top/right/bottom/left/none"));
                }
                put(k, s.into())
            }
            "margins" => {
                let a = arr(v, &p)?;
                if a.len() != 4 {
                    return Err(err(&p, "需要 [上,右,下,左] 四个像素值"));
                }
                for (name, v) in ["top", "right", "bottom", "left"].iter().zip(a) {
                    let n = as_u64(v, &p)?;
                    if n > 16384 {
                        return Err(err(&p, "边距不能超过 16384"));
                    }
                    put(&format!("margin.{name}"), n.to_string())
                }
            }
            "pie_rotation" | "pie_depth" => {
                let n = v.as_f64().ok_or_else(|| err(&p, "需要数值"))?;
                if !(0.0..=360.0).contains(&n) {
                    return Err(err(&p, "范围 0..=360"));
                }
                put(k, n.to_string())
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}
