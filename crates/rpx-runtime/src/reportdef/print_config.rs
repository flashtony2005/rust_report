use super::{
    heads::address_to_rc,
    json::{arr, as_str, err, field, obj},
    DefError,
};
use rpx_core::ReportDefine;
use serde_json::{Map, Value as J};

pub(super) fn apply(rd: &mut ReportDefine, root: &Map<String, J>) -> Result<(), DefError> {
    super::print_options::apply(rd, root)?;
    if let Some(expr) = root
        .get("print_setup")
        .and_then(|p| p.get("row_num_per_page"))
        .and_then(J::as_str)
    {
        if !expr.starts_with('=') {
            return Err(err(
                "$.print_setup.row_num_per_page",
                "动态行数表达式必须以 = 开头",
            ));
        }
        rd.custom_properties
            .push(("rust_report.print.rows".into(), Some(expr.into())));
    }
    if let Some(breaks) = root.get("group_breaks") {
        for (i, item) in arr(breaks, "$.group_breaks")?.iter().enumerate() {
            let path = format!("$.group_breaks[{i}]");
            let item = obj(item, &path)?;
            let address = as_str(field(item, "cell", &path)?, &format!("{path}.cell"))?;
            let (r, c) =
                address_to_rc(address).ok_or_else(|| err(&format!("{path}.cell"), "需要格号"))?;
            if rd.cell_at(r, c).is_none() {
                return Err(err(&format!("{path}.cell"), "组主格不存在"));
            }
            let condition = match item.get("when") {
                None => "true".into(),
                Some(J::Bool(v)) => v.to_string(),
                Some(J::String(expr)) if expr.starts_with('=') => expr.clone(),
                _ => {
                    return Err(err(
                        &format!("{path}.when"),
                        "需要布尔值或以 = 开头的表达式",
                    ))
                }
            };
            rd.custom_properties.push((
                format!("rust_report.print.group.{}", address.to_ascii_uppercase()),
                Some(condition),
            ));
        }
    }
    Ok(())
}
