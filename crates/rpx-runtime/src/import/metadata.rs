use super::*;
use rpx_core::reader::ObjValue;
use std::collections::BTreeMap;
pub(super) fn apply(rd: &ReportDefine, j: &mut J) -> Result<()> {
    let mut rows = Vec::new();
    for (i, r) in rd.row_cells.iter().enumerate() {
        let mut p = json!({"row":i+1,"height":r.row_height,"visible":r.row_visible,"break_page":r.break_page,"break_column":r.break_column,"adjust_height":r.adjust_row_h});
        p["type"] = json!(match r.row_type {
            -93 => "page_header",
            -96 => "title",
            -95 => "table_header",
            -91 => "data",
            -90 => "group_header",
            -94 => "table_footer",
            -89 => "tail_title",
            -92 => "page_footer",
            _ => return Err(unsupported(&format!("/row_props/{i}/type"))),
        });
        if r.row_type == -90 && r.group_header_level > 0 {
            p["group_level"] = json!(r.group_header_level);
        }
        for (key, v) in r.exp_map.as_deref().unwrap_or(&[]) {
            let name = match key {
                1 => "height",
                2 => "visible",
                7 => "adjust_height",
                13 => "break_page",
                15 => "break_column",
                _ => return Err(unsupported(&format!("/row_props/{i}/expressions/{key}"))),
            };
            if let ObjValue::Str(v) = v {
                p[name] = json!(v);
            } else {
                return Err(unsupported(&format!("/row_props/{i}/{name}")));
            }
        }
        rows.push(p);
    }
    j["row_props"] = json!(rows);
    let mut cols = Vec::new();
    for (i, c) in rd.col_cells.iter().enumerate() {
        let mut p = json!({"col":i+1,"width":c.col_width,"visible":c.col_visible,"break_page":c.break_page,"break_column":c.break_column});
        p["type"] = json!(match c.col_type {
            -80 => "left_header",
            -79 => "data",
            -78 => "right_header",
            _ => return Err(unsupported(&format!("/col_props/{i}/type"))),
        });
        for (key, v) in c.exp_map.as_deref().unwrap_or(&[]) {
            let name = match key {
                11 => "width",
                12 => "visible",
                13 => "break_page",
                15 => "break_column",
                _ => return Err(unsupported(&format!("/col_props/{i}/expressions/{key}"))),
            };
            if let ObjValue::Str(v) = v {
                p[name] = json!(v);
            } else {
                return Err(unsupported(&format!("/col_props/{i}/{name}")));
            }
        }
        cols.push(p);
    }
    j["col_props"] = json!(cols);
    let ps = &rd.print_setup;
    let defaults = rpx_core::print_setup::PrintSetup::default_values();
    let mut p = json!({});
    macro_rules! field {($($key:ident),*)=>{$(if ps.$key!=defaults.$key {p[stringify!($key)]=json!(ps.$key);})*};}
    field!(
        zoom_scale,
        table_column_num,
        layout_rows,
        layout_cols,
        row_num_per_page,
        copy_count,
        left_margin,
        right_margin,
        top_margin,
        bottom_margin,
        back_graph_printed,
        column_repeated_group_header,
        full_screen_preview,
        blank_line_filled
    );
    if ps.paper == 256 {
        p["paper_width"] = json!(ps.paper_width);
        p["paper_height"] = json!(ps.paper_height);
    }
    if ps.pager_style != defaults.pager_style {
        p["pager_style"] = json!(match ps.pager_style {
            0 => "none",
            1 => "rows",
            2 => "paper",
            _ => return Err(unsupported("/print_setup/pager_style")),
        });
    }
    if let Some(v) = &ps.virtual_printer {
        p["virtual_printer"] = json!(v);
    }
    j["print_setup"] = p;
    let mut params = Vec::new();
    for (i, p) in rd.params.iter().enumerate() {
        let dt = match p.data_type {
            1 => "int",
            2 => "long",
            3 => "short",
            5 => "float",
            6 => "double",
            7 => "bigdecimal",
            11 => "string",
            12 => "bool",
            51 => "int[]",
            61 => "string[]",
            _ => return Err(unsupported(&format!("/params/{i}/data_type"))),
        };
        let kind = match p.param_type {
            0 => "static",
            1 => "dynamic",
            2 => "attribute",
            _ => return Err(unsupported(&format!("/params/{i}/kind"))),
        };
        let mut value = json!({"name":p.name,"data_type":dt,"kind":kind});
        if let Some(v) = &p.value {
            value["default"] = json!(v);
        }
        if let Some(v) = &p.description {
            value["description"] = json!(v);
        }
        params.push(value);
    }
    j["params"] = json!(params);
    let mut graphics: BTreeMap<(usize, usize), BTreeMap<String, String>> = BTreeMap::new();
    for (k, v) in &rd.custom_properties {
        let v = v
            .as_deref()
            .ok_or_else(|| unsupported(&format!("/custom_properties/{k}")))?;
        if let Some(name) = k.strip_prefix("rust_report.macro.") {
            if j["macros"].is_null() {
                j["macros"] = json!({});
            }
            j["macros"][name] = json!(v);
        } else if let Some(name) = k.strip_prefix(rpx_engine::pager::options::PREFIX) {
            if j["printing"].is_null() {
                j["printing"] = json!({});
            }
            j["printing"][name] = serde_json::from_str(v).unwrap_or_else(|_| json!(v));
        } else if k == "rust_report.print.rows" {
            j["print_setup"]["row_num_per_page"] = json!(v);
        } else if let Some(name) = k.strip_prefix("rust_report.print.group.") {
            if j["group_breaks"].is_null() {
                j["group_breaks"] = json!([]);
            }
            j["group_breaks"].as_array_mut().unwrap().push(json!({"cell":name,"when":serde_json::from_str::<J>(v).unwrap_or_else(|_|json!(v))}));
        } else if k == rpx_engine::graphics::BACKGROUND {
            j["background_image"] = json!(v);
        } else if let Some(key) = k.strip_prefix(rpx_engine::graphics::PREFIX) {
            let mut parts = key.splitn(3, '.');
            let r = parts
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| unsupported(k))?;
            let c = parts
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| unsupported(k))?;
            let key = parts.next().ok_or_else(|| unsupported(k))?;
            graphics
                .entry((r, c))
                .or_default()
                .insert(key.into(), v.into());
        } else {
            return Err(unsupported(&format!("/custom_properties/{k}")));
        }
    }
    for ((r, c), values) in graphics {
        super::visual::apply(j, r, c, &values)?;
    }
    Ok(())
}
