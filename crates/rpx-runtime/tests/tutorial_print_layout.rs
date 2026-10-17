#![cfg(feature = "legacy-rpx")]
use rpx_engine::{
    arena::CalcGrid,
    pager::{paginate_with_setup, project_page},
    value::Value,
};
use serde_json::{json, Value as J};
fn calc(doc: J) -> CalcGrid {
    let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    let rd = rpx_core::open_bytes(&rpx_core::to_bytes(&rd).unwrap()).unwrap();
    rpx_engine::calculate(&rd, &[]).unwrap()
}
fn page_values(g: &CalcGrid) -> Vec<i32> {
    g.cells
        .iter()
        .filter_map(|c| match c.value {
            Value::Int(n) => Some(n),
            _ => None,
        })
        .collect()
}
#[test]
fn columns_flow_down_then_across_and_share_the_same_export_layout() {
    let g = calc(
        json!({"grid":{"rows":1,"cols":1},"cells":[{"row":1,"col":1,"expr":"to(1,8)"}],
        "print_setup":{"pager_style":"rows","row_num_per_page":2,"table_column_num":2},"printing":{"column_gap":7}}),
    );
    let p = paginate_with_setup(&g, &g.print_setup).unwrap();
    assert_eq!(p.page_count, 2);
    assert_eq!(p.columns, 2);
    let first = project_page(&g, &p.pages[0]);
    assert_eq!(page_values(&first), [1, 3, 2, 4]);
    let second = project_page(&g, &p.pages[1]);
    assert_eq!(page_values(&second), [5, 7, 6, 8]);
    use calamine::{Reader, Xlsx};
    let bytes = rpx_export::to_print_xlsx(&g, &g.print_setup).unwrap();
    let mut wb = Xlsx::new(std::io::Cursor::new(bytes)).unwrap();
    let s = wb.worksheet_range("Page 1").unwrap();
    let mut numbers = s
        .rows()
        .flat_map(|r| r.iter())
        .filter_map(|v| {
            if let calamine::Data::Float(n) = v {
                Some(*n as i32)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    numbers.sort();
    assert_eq!(numbers, [1, 2, 3, 4]);
}
#[test]
fn multi_column_rejects_hidden_rows_instead_of_losing_records() {
    let g = calc(
        json!({"grid":{"rows":1,"cols":1},"cells":[{"row":1,"col":1,"expr":"to(1,4)"}],"row_props":[{"row":1,"visible":false}],
        "print_setup":{"table_column_num":2}}),
    );
    assert!(paginate_with_setup(&g, &g.print_setup)
        .unwrap_err()
        .to_string()
        .contains("隐藏行"));
}
fn small_printing(printing: J) -> J {
    json!({"grid":{"rows":4,"cols":1},"cells":(1..=4).map(|r|json!({"row":r,"col":1,"expr":r.to_string()})).collect::<Vec<_>>(),
    "row_props":(1..=4).map(|r|json!({"row":r,"height":5})).collect::<Vec<_>>(),
    "print_setup":{"paper_width":40,"paper_height":40,"left_margin":0,"right_margin":0,"top_margin":0,"bottom_margin":0},"printing":printing})
}
#[test]
fn remaining_space_can_stretch_the_last_row_average_or_proportionally() {
    for (mode, heights) in [
        ("none", vec![5., 5., 5., 5.]),
        ("last", vec![5., 5., 5., 25.]),
        ("average", vec![10., 10., 10., 10.]),
        ("proportional", vec![10., 10., 10., 10.]),
    ] {
        let g = calc(small_printing(json!({"end_space":mode})));
        let p = paginate_with_setup(&g, &g.print_setup).unwrap();
        let page = project_page(&g, &p.pages[0]);
        for (a, b) in page.row_heights.iter().zip(heights) {
            assert!((*a - b).abs() < 0.001, "{mode}: {:?}", page.row_heights)
        }
    }
}
#[test]
fn filling_blank_rows_and_zero_display_do_not_change_calculated_values() {
    let mut doc = small_printing(json!({"zero_as_blank":true,"empty_value":"—"}));
    doc["cells"][0]["expr"] = json!("0");
    doc["print_setup"]["blank_line_filled"] = json!(true);
    let g = calc(doc);
    let p = paginate_with_setup(&g, &g.print_setup).unwrap();
    let page = project_page(&g, &p.pages[0]);
    assert_eq!(g.get(1, 1).unwrap().value, Value::Int(0));
    assert!(g.get(1, 1).unwrap().disp_value.is_none());
    assert_eq!(page.get(1, 1).unwrap().disp_value.as_deref(), Some(""));
    assert_eq!(page.rows, 8);
}
#[test]
fn disabling_an_axis_keeps_it_on_one_sheet() {
    let mut doc = small_printing(json!({"vertical_split":false,"horizontal_split":false}));
    doc["print_setup"]["paper_height"] = json!(6);
    let g = calc(doc);
    let p = paginate_with_setup(&g, &g.print_setup).unwrap();
    assert_eq!(p.page_count, 1);
    assert_eq!(p.pages[0].row_indices, [1, 2, 3, 4]);
}
#[test]
fn printing_modes_control_headers_in_both_directions() {
    let g = calc(
        json!({"grid":{"rows":4,"cols":2},"cells":[{"row":1,"col":1,"text":"标题"},{"row":2,"col":1,"text":"表头"},{"row":3,"col":2,"text":"保留第二列"}],
        "row_props":[{"row":1,"type":"title","height":2},{"row":2,"type":"table_header","height":2},{"row":3,"height":5},{"row":4,"height":5}],
        "col_props":[{"col":1,"width":10},{"col":2,"width":10}],
        "print_setup":{"paper_width":10,"paper_height":10,"left_margin":0,"right_margin":0,"top_margin":0,"bottom_margin":0},
        "printing":{"title_y":"first","title_x":"first","row_header_every":false}}),
    );
    let p = paginate_with_setup(&g, &g.print_setup).unwrap();
    assert_eq!((p.x_pages, p.y_pages), (2, 2));
    assert_eq!(p.pages[0].row_indices, [1, 2, 3]);
    assert_eq!(p.pages[1].row_indices, [2, 3]);
    assert_eq!(p.pages[2].row_indices, [4]);
}

#[test]
fn whole_headers_repeat_while_sliced_headers_follow_horizontal_data() {
    for (mode, expected) in [("every", vec!["左", "右"]), ("every_slice", vec!["右"])] {
        let g = calc(
            json!({"grid":{"rows":2,"cols":2},"cells":[{"row":1,"col":1,"text":"左"},{"row":1,"col":2,"text":"右"}],
            "row_props":[{"row":1,"type":"page_header","height":2}],"col_props":[{"col":1,"width":10},{"col":2,"width":10}],
            "print_setup":{"paper_width":10,"left_margin":0,"right_margin":0},"printing":{"header_x":mode}}),
        );
        let pg = paginate_with_setup(&g, &g.print_setup).unwrap();
        assert_eq!(pg.x_pages, 2);
        let second = project_page(&g, &pg.pages[1]);
        let values = second
            .cells
            .iter()
            .filter_map(|c| {
                if let Value::Text(s) = &c.value {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(values, expected, "{mode}");
        assert!((second.col_widths.iter().sum::<f32>() - 10.0).abs() < 0.001);
    }
}

#[test]
fn merged_title_repeats_as_a_whole_and_split_flag_controls_data_blocks() {
    for (split, pages, first_rows) in [(false, 2, vec![1, 2]), (true, 2, vec![1, 2, 3])] {
        let g = calc(json!({"grid":{"rows":4,"cols":2},"cells":[
            {"row":1,"col":1,"text":"完整标题","merge":{"rows":1,"cols":2}},
            {"row":3,"col":1,"text":"连续块","split":split,"merge":{"rows":2,"cols":1}}],
            "row_props":[{"row":1,"type":"title"}],"col_props":[{"col":1,"width":10},{"col":2,"width":10}],
            "print_setup":{"pager_style":"rows","row_num_per_page":2,"paper_width":10,"left_margin":0,"right_margin":0}}));
        let pg = paginate_with_setup(&g, &g.print_setup).unwrap();
        assert_eq!(pg.y_pages, pages);
        assert_eq!(pg.pages[0].row_indices, first_rows);
        let second_x = project_page(&g, &pg.pages[1]);
        assert!(second_x
            .cells
            .iter()
            .any(|c| c.value == Value::Text("完整标题".into())));
    }
}

#[test]
fn output_empty_preserves_blank_fields_in_printed_sheets_only() {
    let g = calc(
        json!({"grid":{"rows":1,"cols":2},"cells":[{"row":1,"col":2,"text":"有值"}],"printing":{"output_empty":true}}),
    );
    let pg = paginate_with_setup(&g, &g.print_setup).unwrap();
    let page = project_page(&g, &pg.pages[0]);
    assert!(g.get(1, 1).is_none());
    assert_eq!(page.cells.len(), 2);
    assert_eq!(page.get(1, 1).unwrap().disp_value.as_deref(), Some(""));
}
