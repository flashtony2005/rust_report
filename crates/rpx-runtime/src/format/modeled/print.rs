//! 打印设置投影。

use rpx_core::{print_setup::PrintSetup, ReportDefine};
use serde_json::{json, Map, Value as J};

pub(super) fn print_setup(rd: &ReportDefine, warnings: &mut Vec<String>) -> Option<J> {
    let ps = &rd.print_setup;
    let defaults = PrintSetup::default_values();
    let mut p = Map::new();
    let mut put = |name: &str, value: J, differs: bool| {
        if differs {
            p.insert(name.into(), value);
        }
    };
    put(
        "zoom_scale",
        json!(ps.zoom_scale),
        ps.zoom_scale != defaults.zoom_scale,
    );
    put(
        "table_column_num",
        json!(ps.table_column_num),
        ps.table_column_num != defaults.table_column_num,
    );
    put(
        "layout_rows",
        json!(ps.layout_rows),
        ps.layout_rows != defaults.layout_rows,
    );
    put(
        "layout_cols",
        json!(ps.layout_cols),
        ps.layout_cols != defaults.layout_cols,
    );
    put(
        "row_num_per_page",
        json!(ps.row_num_per_page),
        ps.row_num_per_page != defaults.row_num_per_page,
    );
    put(
        "copy_count",
        json!(ps.copy_count),
        ps.copy_count != defaults.copy_count,
    );
    for (name, value, default) in [
        ("left_margin", ps.left_margin, defaults.left_margin),
        ("right_margin", ps.right_margin, defaults.right_margin),
        ("top_margin", ps.top_margin, defaults.top_margin),
        ("bottom_margin", ps.bottom_margin, defaults.bottom_margin),
    ] {
        put(name, json!(value), value != default);
    }
    for (name, value, default) in [
        (
            "back_graph_printed",
            ps.back_graph_printed,
            defaults.back_graph_printed,
        ),
        (
            "column_repeated_group_header",
            ps.column_repeated_group_header,
            defaults.column_repeated_group_header,
        ),
        (
            "full_screen_preview",
            ps.full_screen_preview,
            defaults.full_screen_preview,
        ),
        (
            "blank_line_filled",
            ps.blank_line_filled,
            defaults.blank_line_filled,
        ),
    ] {
        put(name, json!(value), value != default);
    }
    if ps.paper == 256 {
        put("paper_width", json!(ps.paper_width), true);
        put("paper_height", json!(ps.paper_height), true);
    } else if ps.paper != defaults.paper {
        warnings.push("print_setup/paper".into());
    }
    if let Some(style) = match ps.pager_style {
        0 => Some("none"),
        1 => Some("rows"),
        2 => Some("paper"),
        _ => None,
    } {
        put(
            "pager_style",
            json!(style),
            ps.pager_style != defaults.pager_style,
        );
    } else {
        warnings.push("print_setup/pager_style".into());
    }
    if let Some(v) = &ps.virtual_printer {
        p.insert("virtual_printer".into(), json!(v));
    }
    // 其余 33 槽：非默认时进 legacy。
    if ps.paper == 256 {
        // custom paper 已表达；paper 码本身不需要。
    }
    for (name, differs) in [
        ("paper", ps.paper != defaults.paper && ps.paper != 256),
        ("orient", ps.orient != defaults.orient),
        (
            "zoom_page_width",
            ps.zoom_page_width != defaults.zoom_page_width,
        ),
        (
            "zoom_page_height",
            ps.zoom_page_height != defaults.zoom_page_height,
        ),
        ("zoom_mode", ps.zoom_mode != defaults.zoom_mode),
        ("layout", ps.layout != defaults.layout),
        ("h_align", ps.h_align != defaults.h_align),
        ("v_align", ps.v_align != defaults.v_align),
        (
            "page_header_and_footer_y_mode",
            ps.page_header_and_footer_y_mode != defaults.page_header_and_footer_y_mode,
        ),
        (
            "page_header_and_footer_x_mode",
            ps.page_header_and_footer_x_mode != defaults.page_header_and_footer_x_mode,
        ),
        (
            "row_table_header_and_footer_mode",
            ps.row_table_header_and_footer_mode != defaults.row_table_header_and_footer_mode,
        ),
        ("title_y_mode", ps.title_y_mode != defaults.title_y_mode),
        ("title_x_mode", ps.title_x_mode != defaults.title_x_mode),
        (
            "col_table_header_and_footer_mode",
            ps.col_table_header_and_footer_mode != defaults.col_table_header_and_footer_mode,
        ),
        ("column_mode", ps.column_mode != defaults.column_mode),
    ] {
        if differs {
            warnings.push(format!("print_setup/{name}"));
        }
    }
    if p.is_empty() {
        None
    } else {
        Some(J::Object(p))
    }
}
