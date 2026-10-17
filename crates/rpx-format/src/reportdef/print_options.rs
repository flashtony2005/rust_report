//! 可读的打印规则与打印方式矩阵；与产品 33 槽的数值码表分开保存。
use super::{
    json::{as_str, err, obj},
    DefError,
};
use rpx_core::ReportDefine;
use rpx_engine::pager::options::{Repeat, PREFIX};
use serde_json::{Map, Value as J};

pub(super) fn apply(rd: &mut ReportDefine, root: &Map<String, J>) -> Result<(), DefError> {
    let Some(value) = root.get("printing") else {
        return Ok(());
    };
    for (key, value) in obj(value, "$.printing")? {
        let path = format!("$.printing.{key}");
        let text = match key.as_str() {
            "auto_size"
            | "horizontal_split"
            | "vertical_split"
            | "wrap_numbers"
            | "zero_as_blank"
            | "output_empty"
            | "row_header_every"
            | "row_footer_every"
            | "column_header_every"
            | "column_footer_every" => value
                .as_bool()
                .ok_or_else(|| err(&path, "需要布尔值"))?
                .to_string(),
            "empty_value" => as_str(value, &path)?.into(),
            "border_width" | "column_gap" => {
                let n = value.as_f64().ok_or_else(|| err(&path, "需要非负数"))?;
                if !(0.0..=1000.0).contains(&n) {
                    return Err(err(&path, "范围 0..=1000"));
                }
                n.to_string()
            }
            "border_color" => {
                let s = as_str(value, &path)?;
                if s.len() != 7 || !s.starts_with('#') || u32::from_str_radix(&s[1..], 16).is_err()
                {
                    return Err(err(&path, "需要 #RRGGBB"));
                }
                s.into()
            }
            "end_space" => {
                let s = as_str(value, &path)?;
                if !["none", "last", "average", "proportional"].contains(&s) {
                    return Err(err(&path, "可选 none / last / average / proportional"));
                }
                s.into()
            }
            "header_y" | "title_y" | "header_x" | "title_x" => {
                let s = as_str(value, &path)?;
                if Repeat::parse(s).is_none() {
                    return Err(err(&path, "可选 first / every / every_slice / first_slice"));
                }
                if key.ends_with("_y") && !["first", "every"].contains(&s) {
                    return Err(err(&path, "纵向可选 first / every"));
                }
                s.into()
            }
            _ => return Err(err(&path, "未知打印规则")),
        };
        rd.custom_properties
            .push((format!("{PREFIX}{key}"), Some(text)));
    }
    Ok(())
}
