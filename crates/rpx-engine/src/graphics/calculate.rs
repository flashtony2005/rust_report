use super::{Graphic, GraphicContent, PREFIX};
use crate::{
    calc::{ctx::CalcCtx, ds_current::set_ds_current_for_cell},
    error::{EngineError, Result},
    expr::{eval::EvalCtx, evaluate, parse_with},
    macros::{substitute, MacroTable},
    params::ParamTable,
    value::Value,
};
use rpx_core::ReportDefine;
use std::collections::{BTreeMap, HashSet};

pub(super) fn invalid(detail: impl Into<String>) -> EngineError {
    EngineError::UnsupportedExpr {
        expr: format!("图形：{}", detail.into()),
    }
}

pub(crate) fn collect(
    ctx: &mut CalcCtx,
    rd: &ReportDefine,
    params: &ParamTable,
    macros: &MacroTable,
) -> Result<Vec<Graphic>> {
    let mut definitions = BTreeMap::<(i32, i32), BTreeMap<String, String>>::new();
    for (key, value) in &rd.custom_properties {
        let Some(key) = key.strip_prefix(PREFIX) else {
            continue;
        };
        let mut parts = key.splitn(3, '.');
        let row = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| invalid("缺少行号"))?;
        let col = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| invalid("缺少列号"))?;
        let field = parts.next().ok_or_else(|| invalid("缺少字段"))?;
        definitions
            .entry((row, col))
            .or_default()
            .insert(field.into(), value.clone().unwrap_or_default());
    }
    if definitions.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let ids: Vec<_> = ctx.grid.by_pos.values().copied().collect();
    for id in ids {
        if !seen.insert(id) {
            continue;
        }
        let Some(host) = ctx.grid.get(id) else {
            continue;
        };
        let Some(def) = definitions.get(&(host.src_row, host.src_col)) else {
            continue;
        };
        let (row, col) = (host.row, host.col);
        let (row_span, col_span) = (host.row_merge, host.col_merge);
        ctx.set_current(id);
        ctx.save_current();
        set_ds_current_for_cell(ctx, id)?;
        let field = |key: &str| {
            def.get(key)
                .map(String::as_str)
                .ok_or_else(|| invalid(format!("缺少 {key}")))
        };
        let width: u32 = field("width")?.parse().map_err(|_| invalid("width"))?;
        let height: u32 = field("height")?.parse().map_err(|_| invalid("height"))?;
        if !(1..=16384).contains(&width) || !(1..=16384).contains(&height) {
            return Err(invalid("图形尺寸应在 1..=16384 像素内"));
        }
        let content = match field("type")? {
            "image" => GraphicContent::Image(image(eval(field("source")?, ctx, params, macros)?)?),
            "cell_background" => {
                GraphicContent::CellBackground(image(eval(field("source")?, ctx, params, macros)?)?)
            }
            "echarts" | "echarts_js" => {
                let mut table = BTreeMap::new();
                table.insert(
                    "id".into(),
                    crate::emit::render_value(&Value::Text("chart".into())),
                );
                table.insert("width".into(), width.to_string());
                table.insert("height".into(), height.to_string());
                for (key, expr) in def {
                    if let Some(name) = key.strip_prefix("param.") {
                        table.insert(
                            name.to_string(),
                            crate::emit::render_value(&eval(expr, ctx, params, macros)?),
                        );
                    }
                }
                let source = template(field("template")?, &table)?;
                if field("type")? == "echarts_js" {
                    GraphicContent::EchartsScript(source)
                } else {
                    GraphicContent::Echarts(source)
                }
            }
            name => GraphicContent::Chart(Box::new(super::chart::calculate(
                name, def, ctx, params, macros,
            )?)),
        };
        out.push(Graphic {
            row_span,
            col_span,
            row,
            col,
            width,
            height,
            content,
        });
    }
    out.sort_by_key(|g| (g.row, g.col));
    Ok(out)
}

pub(super) fn eval(
    text: &str,
    ctx: &mut CalcCtx,
    params: &ParamTable,
    macros: &MacroTable,
) -> Result<Value> {
    match text.strip_prefix('=') {
        Some(expr) => evaluate(&parse_with(&substitute(expr, macros)?, params)?, ctx),
        None => Ok(Value::Text(text.into())),
    }
}
pub(super) fn list(value: Value) -> Vec<Value> {
    match value {
        Value::List(v) => v,
        Value::Null => Vec::new(),
        v => vec![v],
    }
}
fn image(value: Value) -> Result<String> {
    if let Value::Text(uri) = value {
        if [
            "data:image/png;base64,",
            "data:image/jpeg;base64,",
            "data:image/gif;base64,",
            "data:image/bmp;base64,",
        ]
        .iter()
        .any(|p| uri.starts_with(p))
        {
            return Ok(uri);
        }
    }
    Err(invalid("图片应为 PNG/JPEG/GIF/BMP base64 data URI"))
}

// 模板参数只对原模板扫描一次，参数文本里的 `${...}` 作为数据原样保留。
fn template(source: &str, values: &BTreeMap<String, String>) -> Result<String> {
    let mut out = String::new();
    let mut rest = source;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let end = tail.find('}').ok_or_else(|| invalid("模板参数缺少 }"))?;
        let name = &tail[..end];
        out.push_str(
            values
                .get(name)
                .ok_or_else(|| invalid(format!("未定义模板参数 {name}")))?,
        );
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}
