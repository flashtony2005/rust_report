//! JSON 图形契约写入现有自定义属性；不伪造产品的序列化对象。
use super::{
    json::{arr, as_str, as_u64, err, field, obj, opt_str},
    DefError,
};
use rpx_core::ReportDefine;
use rpx_engine::graphics::{BACKGROUND, PREFIX};
use serde_json::{Map, Value as J};

pub(super) fn apply(rd: &mut ReportDefine, root: &Map<String, J>) -> Result<(), DefError> {
    if let Some(uri) = opt_str(root, "background_image", "$")? {
        check_image(uri, "$.background_image")?;
        rd.custom_properties
            .push((BACKGROUND.into(), Some(uri.into())));
    }
    let Some(cells) = root.get("cells") else {
        return Ok(());
    };
    for (i, cell) in arr(cells, "$.cells")?.iter().enumerate() {
        let path = format!("$.cells[{i}]");
        let cell = obj(cell, &path)?;
        let Some(kind) = opt_str(cell, "data_type", &path)? else {
            continue;
        };
        if kind == "normal" {
            continue;
        }
        let row = as_u64(field(cell, "row", &path)?, &path)?;
        let col = as_u64(field(cell, "col", &path)?, &path)?;
        let key = match kind {
            "chart" => "chart",
            "image" | "cell_background" => "image",
            "echarts" | "echarts_js" => "echarts",
            _ => {
                return Err(err(
                    &format!("{path}.data_type"),
                    "可选 normal / chart / image / cell_background / echarts / echarts_js",
                ))
            }
        };
        let path = format!("{path}.{key}");
        let o = obj(field(cell, key, &path)?, &path)?;
        let mut put = |name: &str, value: String| {
            rd.custom_properties
                .push((format!("{PREFIX}{row}.{col}.{name}"), Some(value)))
        };
        for (name, default) in [("width", 480), ("height", 280)] {
            let n = o
                .get(name)
                .map(|v| as_u64(v, &format!("{path}.{name}")))
                .transpose()?
                .unwrap_or(default);
            if !(1..=16384).contains(&n) {
                return Err(err(&format!("{path}.{name}"), "尺寸应在 1..=16384 像素内"));
            }
            put(name, n.to_string());
        }
        match kind {
            "chart" => super::chart::apply(o, &path, &mut put)?,
            "image" | "cell_background" => {
                unknown(o, &["source", "width", "height"], &path)?;
                let source = as_str(field(o, "source", &path)?, &format!("{path}.source"))?;
                if !source.starts_with('=') {
                    check_image(source, &format!("{path}.source"))?
                }
                put("type", kind.into());
                put("source", source.into());
            }
            "echarts" | "echarts_js" => {
                unknown(o, &["template", "params", "width", "height"], &path)?;
                put("type", kind.into());
                put(
                    "template",
                    as_str(field(o, "template", &path)?, &format!("{path}.template"))?.into(),
                );
                if let Some(params) = o.get("params") {
                    for (name, value) in obj(params, &format!("{path}.params"))? {
                        if name.is_empty()
                            || name.contains(['{', '}'])
                            || ["id", "width", "height"].contains(&name.as_str())
                        {
                            return Err(err(&format!("{path}.params"), "模板参数名不合法"));
                        }
                        put(
                            &format!("param.{name}"),
                            expression(value, &format!("{path}.params.{name}"))?,
                        );
                    }
                }
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}
pub(super) fn expression(v: &J, path: &str) -> Result<String, DefError> {
    let s = as_str(v, path)?;
    if !s.starts_with('=') {
        return Err(err(path, "图形绑定表达式必须以 = 开头"));
    }
    Ok(s.into())
}
pub(super) fn unknown(o: &Map<String, J>, names: &[&str], path: &str) -> Result<(), DefError> {
    for key in o.keys() {
        if !names.contains(&key.as_str()) {
            return Err(err(&format!("{path}.{key}"), "未知图形字段"));
        }
    }
    Ok(())
}
fn check_image(uri: &str, path: &str) -> Result<(), DefError> {
    rpx_export::validate_image(uri).map_err(|e| err(path, e))
}
