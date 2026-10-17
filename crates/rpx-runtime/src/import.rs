//! Conservative RPX import: reconstruction must preserve every modeled field.
use crate::{
    definition,
    error::{Error, Result},
    reportdef,
};
use rpx_core::{props::bit, ReportDefine};
use serde_json::{json, Value as J};

mod metadata;
mod visual;

fn unsupported(path: &str) -> Error {
    Error::new(
        3,
        "RPX_IMPORT_UNSUPPORTED",
        "import",
        format!("JSON 尚不能无损表达 {path}"),
    )
    .at(path)
}
pub fn convert(bytes: &[u8]) -> Result<J> {
    let rd = rpx_core::open_bytes(bytes).map_err(|e| Error::new(3, "RPX_INVALID", "import", e))?;
    let mut cells = Vec::new();
    for row in 1..rd.rows {
        for col in 1..rd.cols {
            let Some(c) = rd.cell_at(row, col) else {
                continue;
            };
            if c.merged_area
                .is_some_and(|a| a.begin_row as usize != row || a.begin_col as usize != col)
            {
                continue;
            }
            let mut j = json!({"row":row,"col":col});
            if let Some(expr) = rpx_engine::cell_expression(c) {
                j["expr"] = json!(expr);
            } else {
                j["literal"] = reportdef::literal::encode(&c.value).ok_or_else(|| {
                    unsupported(&format!(
                        "/cells/{}/value",
                        definition::address(row as u64, col as u64)
                    ))
                })?;
            }
            if let Some(d) = &c.disp_value {
                j["disp_value"] = json!(d);
            }
            for (key, value) in [
                ("font_name", &c.font_name),
                ("format", &c.format),
                ("left_head", &c.left_head),
                ("top_head", &c.top_head),
            ] {
                if let Some(v) = value {
                    j[key] = json!(v);
                }
            }
            j["font_size"] = json!(c.font_size);
            j["fore_color"] = json!(format!("#{:06X}", c.fore_color as u32 & 0xffffff));
            if c.back_color != -1 {
                j["back_color"] = json!(format!("#{:06X}", c.back_color as u32 & 0xffffff));
            }
            for (k, flag) in [
                ("bold", bit::BOLD),
                ("italic", bit::ITALIC),
                ("underline", bit::UNDERLINE),
                ("split", bit::SPLITTED),
                ("stretch_when_paged", bit::STRETCH_WHEN_PAGED),
            ] {
                j[k] = json!(c.bit_flags & flag != 0);
            }
            j["indent"] = json!(c.indent);
            j["rotation"] = json!(c.rotation_angle);
            j["extend_mode"] = json!(match c.extend_mode {
                16 => "default",
                17 => "none",
                18 => "vertical",
                19 => "horizontal",
                _ => return Err(unsupported("/cells/extend_mode")),
            });
            j["h_align"] = json!(match c.h_align {
                -48 => "left",
                -47 => "center",
                -46 => "right",
                -45 => "justify",
                -44 => "distribute",
                _ => return Err(unsupported("/cells/h_align")),
            });
            j["v_align"] = json!(match c.v_align {
                -32 => "top",
                -31 => "middle",
                -30 => "bottom",
                _ => return Err(unsupported("/cells/v_align")),
            });
            if let Some(a) = c.merged_area {
                j["merge"] = json!({"rows":a.end_row-a.begin_row+1,"cols":a.end_col-a.begin_col+1});
            }
            if let Some(mode) = match c.merge_same {
                98 => Some("vertical"),
                99 => Some("horizontal"),
                100 => Some("both"),
                _ => None,
            } {
                j["merge_same"] = json!(mode);
            }
            let mut border = json!({});
            for (k, style, width, color) in [
                ("left", c.lb_style, c.lb_width, c.lb_color),
                ("right", c.rb_style, c.rb_width, c.rb_color),
                ("top", c.tb_style, c.tb_width, c.tb_color),
                ("bottom", c.bb_style, c.bb_width, c.bb_color),
            ] {
                border[k] = json!({"style":match style{0=>"none",81=>"dotted",82=>"dashed",83=>"solid",84=>"double",_=>return Err(unsupported("/cells/border/style"))},"width":width,"color":format!("#{:06X}",color as u32&0xffffff)});
            }
            j["border"] = border;
            cells.push(j);
        }
    }
    let mut datasets = Vec::new();
    for d in &rd.datasets {
        if let Some(f) = &d.file {
            let mut file = json!({"file_name":f.file_name,"delimiter":char::from_u32(f.seperator as u32).ok_or_else(||unsupported("/datasets/delimiter"))?.to_string()});
            if let Some(v) = &f.encoding {
                file["encoding"] = json!(v);
            }
            if let Some(v) = &f.sheet_name {
                file["sheet_name"] = json!(v);
            }
            datasets.push(json!({"name":f.name,"file":file}));
        } else if let Some(s) = &d.sql {
            datasets.push(json!({"name":s.name,"sql":{"dsn":s.data_source_name,"sql":s.sql}}));
        } else {
            return Err(unsupported("/datasets/class_name"));
        }
    }
    let mut value = json!({"version":1,"grid":{"rows":rd.rows-1,"cols":rd.cols-1},"cells":cells,"datasets":datasets});
    if rd.datasets.is_empty() {
        value.as_object_mut().unwrap().remove("datasets");
    }
    metadata::apply(&rd, &mut value)?;
    let (rebuilt, _) = definition::parse(&value).map_err(|e| {
        let path = e.diagnostic["json_pointer"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Error::new(3, "RPX_IMPORT_UNSUPPORTED", "import", e).at(path)
    })?;
    compare(&rd, &rebuilt)?;
    Ok(value)
}
fn compare(a: &ReportDefine, b: &ReportDefine) -> Result<()> {
    macro_rules! check {
        ($field:ident) => {
            if a.$field != b.$field {
                return Err(unsupported(concat!("/", stringify!($field))));
            }
        };
    }
    check!(row_cells);
    check!(col_cells);
    check!(print_setup);
    check!(params);
    let sorted = |rd: &ReportDefine| {
        let mut p = rd.custom_properties.clone();
        p.sort();
        p
    };
    if sorted(a) != sorted(b) {
        return Err(unsupported("/custom_properties"));
    }
    check!(exp_map);
    check!(scalars);
    check!(macro_meta);
    check!(sub_report_meta);
    check!(back_graph_config);
    check!(export_config);
    for (i, (a, b)) in a.datasets.iter().zip(&b.datasets).enumerate() {
        if a.file != b.file || a.sql != b.sql || a.class_name != b.class_name {
            return Err(unsupported(&format!("/datasets/{i}")));
        }
    }
    for r in 1..a.rows {
        for c in 1..a.cols {
            let (a, b) = (a.cell_at(r, c), b.cell_at(r, c));
            if a.is_some_and(|cell| {
                cell.merged_area
                    .is_some_and(|m| m.begin_row as usize != r || m.begin_col as usize != c)
            }) {
                continue;
            }
            match (a, b) {
                (Some(a), Some(b)) => {
                    macro_rules! cell_field {($($key:ident),*)=>{$(if a.$key!=b.$key{return Err(unsupported(&format!("/cells/{}/{}",definition::address(r as u64,c as u64),stringify!($key))));})*};}
                    cell_field!(
                        merged_area,
                        h_align,
                        v_align,
                        diagonal_style,
                        diagonal_color,
                        diagonal_width,
                        extend_mode,
                        extend_area,
                        left_head,
                        top_head,
                        to_excel_mode,
                        adjust_size_mode,
                        cell_obj,
                        value,
                        disp_value,
                        indent,
                        font_name,
                        font_size,
                        format,
                        fore_color,
                        back_color,
                        hyperlink,
                        hyperlink_window,
                        lb_style,
                        lb_width,
                        lb_color,
                        rb_style,
                        rb_width,
                        rb_color,
                        tb_style,
                        tb_width,
                        tb_color,
                        bb_style,
                        bb_width,
                        bb_color,
                        exp_map,
                        bit_flags,
                        cell_type,
                        notes,
                        ois_blob,
                        cell_graph_config,
                        under_line_style,
                        cell_style_name,
                        merge_same,
                        merge_null,
                        merge_same_mode,
                        merge_null_mode,
                        rotation_angle
                    );
                }
                (None, None) => {}
                _ => {
                    return Err(unsupported(&format!(
                        "/cells/{}",
                        definition::address(r as u64, c as u64)
                    )))
                }
            }
        }
    }
    Ok(())
}
