use super::{
    calculate::{eval, invalid, list},
    number, Appearance, Chart, ChartKind, Series, Tasks,
};
use crate::{
    calc::ctx::CalcCtx, error::Result, macros::MacroTable, params::ParamTable, value::Value,
};
use std::collections::BTreeMap;

pub(super) fn calculate(
    name: &str,
    def: &BTreeMap<String, String>,
    ctx: &mut CalcCtx,
    params: &ParamTable,
    macros: &MacroTable,
) -> Result<Chart> {
    let field = |key: &str| {
        def.get(key)
            .map(String::as_str)
            .ok_or_else(|| invalid(format!("缺少 {key}")))
    };
    let kind = ChartKind::parse(name).ok_or_else(|| invalid(format!("不支持 {name}")))?;
    let title = eval(field("title")?, ctx, params, macros)?.to_string();
    let appearance = Appearance {
        colors: def
            .get("colors")
            .map(|s| s.split(',').map(str::to_string).collect())
            .unwrap_or_default(),
        legend: def
            .get("legend")
            .cloned()
            .unwrap_or_else(|| "bottom".into()),
        background: def
            .get("background")
            .cloned()
            .unwrap_or_else(|| "transparent".into()),
        margins: ["margin.top", "margin.right", "margin.bottom", "margin.left"]
            .map(|key| def.get(key).and_then(|s| s.parse().ok()).unwrap_or(40)),
        pie_rotation: def
            .get("pie_rotation")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        pie_depth: def
            .get("pie_depth")
            .and_then(|s| s.parse().ok())
            .unwrap_or(16.0),
    };
    if kind.intervals() {
        let categories = list(eval(field("tasks.categories")?, ctx, params, macros)?);
        let starts = list(eval(field("tasks.starts")?, ctx, params, macros)?);
        let ends = if kind == ChartKind::Milestone {
            starts.clone()
        } else {
            list(eval(field("tasks.ends")?, ctx, params, macros)?)
        };
        let states = match def.get("tasks.states") {
            Some(s) => list(eval(s, ctx, params, macros)?),
            None => vec![Value::Text(String::new()); categories.len()],
        };
        if categories.len() != starts.len()
            || categories.len() != ends.len()
            || categories.len() != states.len()
        {
            return Err(invalid("任务分类、开始、结束、状态的长度必须一致"));
        }
        for v in starts.iter().chain(&ends) {
            if !matches!(v,Value::Text(s) if !s.is_empty())
                && !number(v).is_some_and(f64::is_finite)
            {
                return Err(invalid("任务时间需要时间字符串或有限毫秒时间戳"));
            }
        }
        for (start, end) in starts.iter().zip(&ends) {
            if let (Some(start), Some(end)) = (number(start), number(end)) {
                if end < start {
                    return Err(invalid("结束时间不能早于开始时间"));
                }
            }
        }
        return Ok(Chart {
            kind,
            title,
            categories: Vec::new(),
            series: Vec::new(),
            appearance,
            tasks: Some(Tasks {
                categories,
                starts,
                ends,
                states,
            }),
        });
    }
    let categories = list(eval(field("categories")?, ctx, params, macros)?);
    let count: usize = field("series_count")?
        .parse()
        .map_err(|_| invalid("series_count"))?;
    if !(1..=256).contains(&count) {
        return Err(invalid("需要 1..=256 个系列"));
    }
    let mut series = Vec::new();
    for i in 0..count {
        let name = eval(field(&format!("series.{i}.name"))?, ctx, params, macros)?.to_string();
        let values = list(eval(
            field(&format!("series.{i}.values"))?,
            ctx,
            params,
            macros,
        )?);
        if values
            .iter()
            .any(|v| !v.is_null() && !number(v).is_some_and(f64::is_finite))
        {
            return Err(invalid(format!("系列 {name} 包含非有限数值")));
        }
        if values.len() != categories.len() {
            return Err(invalid(format!(
                "系列 {name} 有 {} 个值，分类轴有 {} 项",
                values.len(),
                categories.len()
            )));
        }
        series.push(Series { name, values });
    }
    if matches!(
        kind,
        ChartKind::Pie | ChartKind::Pie3d | ChartKind::Gauge | ChartKind::Gauge3d
    ) && series.len() != 1
    {
        return Err(invalid("饼图与仪表盘需要恰好一个系列"));
    }
    if matches!(kind, ChartKind::Gauge | ChartKind::Gauge3d) && categories.len() != 1 {
        return Err(invalid("仪表盘需要恰好一个分类值"));
    }
    if matches!(kind, ChartKind::Pie | ChartKind::Pie3d)
        && series[0]
            .values
            .iter()
            .any(|v| number(v).is_some_and(|n| n < 0.0))
    {
        return Err(invalid("饼图数据不能为负数"));
    }
    if kind == ChartKind::Scatter
        && categories
            .iter()
            .any(|v| !number(v).is_some_and(f64::is_finite))
    {
        return Err(invalid("散点图分类轴需要有限数值"));
    }
    if matches!(kind, ChartKind::ErrorBar | ChartKind::Range) {
        if !(2..=3).contains(&series.len()) {
            return Err(invalid(
                "工字/全距图需要下界、上界两个系列，可带第三个中心值系列",
            ));
        }
        for i in 0..categories.len() {
            if let (Some(low), Some(high)) =
                (number(&series[0].values[i]), number(&series[1].values[i]))
            {
                if low > high {
                    return Err(invalid("下界不能大于上界"));
                }
            }
        }
    }
    Ok(Chart {
        kind,
        title,
        categories,
        series,
        appearance,
        tasks: None,
    })
}
