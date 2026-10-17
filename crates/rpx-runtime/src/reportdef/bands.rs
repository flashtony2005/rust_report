//! JSON 行列区带与计算属性，复用 RPX 已有槽位。
use super::{
    build::Builder,
    json::{as_str, as_u64, err},
    parse::bool_or_expr,
    DefError,
};
use serde_json::{Map, Value as J};

pub(super) fn row(
    b: &mut Builder,
    r: usize,
    o: &Map<String, J>,
    path: &str,
) -> Result<(), DefError> {
    if let Some(v) = o.get("type") {
        let p = format!("{path}.type");
        b.row_mut(r).row_type = match as_str(v, &p)? {
            "page_header" => -93,
            "title" => -96,
            "table_header" => -95,
            "data" => -91,
            "group_header" => -90,
            "table_footer" => -94,
            "tail_title" => -89,
            "page_footer" => -92,
            v => return Err(err(&p, format!("未知行类型 {v:?}"))),
        };
    }
    if let Some(v) = o.get("group_level") {
        let p = format!("{path}.group_level");
        let n = as_u64(v, &p)?;
        if !(1..=20).contains(&n) {
            return Err(err(&p, "组头级别应在 1..=20"));
        }
        if b.row_mut(r).row_type != -90 {
            return Err(err(&p, "仅分组表头可设置组头级别"));
        }
        b.row_mut(r).group_header_level = n as i8;
    }
    for (name, key) in [("visible", 2), ("adjust_height", 7), ("break_column", 15)] {
        if let Some(v) = o.get(name) {
            if let Some(v) = v.as_bool() {
                let row = b.row_mut(r);
                match name {
                    "visible" => row.row_visible = v,
                    "adjust_height" => row.adjust_row_h = v,
                    _ => row.break_column = v,
                }
            } else {
                b.row_prop(r, key, &bool_or_expr(v, &format!("{path}.{name}"))?);
            }
        }
    }
    Ok(())
}

pub(super) fn col(
    b: &mut Builder,
    c: usize,
    o: &Map<String, J>,
    path: &str,
) -> Result<(), DefError> {
    if let Some(v) = o.get("type") {
        let p = format!("{path}.type");
        b.col_mut(c).col_type = match as_str(v, &p)? {
            "left_header" => -80,
            "data" => -79,
            "right_header" => -78,
            v => return Err(err(&p, format!("未知列类型 {v:?}"))),
        };
    }
    for (name, key) in [("visible", 12), ("break_page", 13), ("break_column", 15)] {
        if let Some(v) = o.get(name) {
            if let Some(v) = v.as_bool() {
                let col = b.col_mut(c);
                match name {
                    "visible" => col.col_visible = v,
                    "break_page" => col.break_page = v,
                    _ => col.break_column = v,
                }
            } else {
                b.col_prop(c, key, &bool_or_expr(v, &format!("{path}.{name}"))?);
            }
        }
    }
    Ok(())
}
