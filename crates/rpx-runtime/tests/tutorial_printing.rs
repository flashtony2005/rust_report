use rpx_engine::{
    arena::CalcGrid,
    pager::{paginate_with_setup, project_page},
};
use serde_json::{json, Value as J};

fn report(mode: &str) -> (rpx_core::ReportDefine, CalcGrid) {
    let cells: Vec<_> = (1..=8)
        .map(|r| json!({"row":r,"col":1,"text":format!("R{r}")}))
        .collect();
    let rd = rpx_runtime::reportdef::build(&json!({
        "grid":{"rows":8,"cols":1},"cells":cells,
        "row_props":[{"row":1,"type":"table_header"},{"row":7,"type":"table_footer"},{"row":8,"type":"page_footer"},{"row":3,"break_page":true}],
        "print_setup":{"pager_style":mode,"row_num_per_page":2}
    }).to_string()).unwrap();
    let mut grid = rpx_engine::calculate(&rd, &[]).unwrap();
    grid.pagination = Some(paginate_with_setup(&grid, &rd.print_setup).unwrap());
    (rd, grid)
}

#[test]
fn row_paging_repeats_headers_and_footers_but_emits_tail_only_once() {
    let (rd, g) = report("rows");
    let pg = g.pagination.as_ref().unwrap();
    let rows: Vec<_> = pg.pages.iter().map(|p| p.row_indices.clone()).collect();
    assert_eq!(
        rows,
        vec![vec![1, 2, 3, 8], vec![1, 4, 5, 8], vec![1, 6, 7, 8]]
    );
    let wb: J =
        serde_json::from_str(&rpx_runtime::snapshot::build(&rd, &g, "bands").unwrap()).unwrap();
    let second = &wb["sheets"]["rpx-sheet-2"]["cellData"];
    assert_eq!(second["0"]["0"]["v"], "R1");
    assert_eq!(second["1"]["0"]["v"], "R4");
    assert_eq!(second["2"]["0"]["v"], "R5");
    assert_eq!(second["3"]["0"]["v"], "R8");
    assert!(second.get("4").is_none());
}

#[test]
fn manual_page_breaks_only_apply_to_paper_mode() {
    let (_, paper) = report("paper");
    assert_eq!(
        paper.pagination.unwrap().pages[0].row_indices,
        vec![1, 2, 3, 8]
    );
    let (_, none) = report("none");
    let pg = none.pagination.unwrap();
    assert_eq!(pg.page_count, 1);
    assert_eq!(pg.pages[0].row_indices, (1..=8).collect::<Vec<_>>());
}

#[test]
fn horizontal_pages_repeat_both_column_bands_and_obey_custom_paper_width() {
    let rd = rpx_runtime::reportdef::build(&json!({
        "grid":{"rows":1,"cols":5},
        "cells":(1..=5).map(|c| json!({"row":1,"col":c,"text":format!("C{c}")})).collect::<Vec<_>>(),
        "col_props":[{"col":1,"type":"left_header","width":5},{"col":2,"width":10},{"col":3,"width":10},{"col":4,"width":10},{"col":5,"type":"right_header","width":5}],
        "print_setup":{"paper_width":25,"paper_height":100,"left_margin":0,"right_margin":0,"top_margin":0,"bottom_margin":0}
    }).to_string()).unwrap();
    let g = rpx_engine::calculate(&rd, &[]).unwrap();
    let pg = paginate_with_setup(&g, &rd.print_setup).unwrap();
    assert_eq!(
        pg.pages
            .iter()
            .map(|p| p.col_indices.clone())
            .collect::<Vec<_>>(),
        vec![vec![1, 2, 5], vec![1, 3, 5], vec![1, 4, 5]]
    );
    let second = project_page(&g, &pg.pages[1]);
    assert_eq!(
        second.get(1, 2).unwrap().value,
        rpx_engine::value::Value::Text("C3".into())
    );
    assert_eq!(
        second.get(1, 3).unwrap().value,
        rpx_engine::value::Value::Text("C5".into())
    );
}

#[test]
fn invalid_row_limit_and_unknown_bands_are_rejected_at_load() {
    for patch in [
        json!({"print_setup":{"pager_style":"rows","row_num_per_page":0}}),
        json!({"row_props":[{"row":1,"type":"typo"}]}),
        json!({"col_props":[{"col":1,"type":"typo"}]}),
    ] {
        let mut doc = json!({"grid":{"rows":1,"cols":1}});
        doc.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(rpx_runtime::reportdef::build(&doc.to_string()).is_err());
    }
}

#[test]
fn printed_xlsx_uses_the_same_page_maps_as_the_preview() {
    use calamine::{Reader, Xlsx};
    let (_, g) = report("rows");
    let mut setup = rpx_core::print_setup::PrintSetup::default_values();
    setup.pager_style = 1;
    setup.row_num_per_page = 2;
    let bytes = rpx_export::to_print_xlsx(&g, &setup).unwrap();
    let mut book = Xlsx::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(book.sheet_names(), vec!["Page 1", "Page 2", "Page 3"]);
    let second = book.worksheet_range("Page 2").unwrap();
    let values: Vec<_> = second.rows().map(|r| r[0].to_string()).collect();
    assert_eq!(values, vec!["R1", "R4", "R5", "R8"]);
}

#[test]
fn dynamic_page_size_survives_rpx_and_resolves_before_pagination() {
    let doc = json!({"grid":{"rows":1,"cols":1},"cells":[{"row":1,"col":1,"expr":"to(1,5)"}],
        "print_setup":{"pager_style":"rows","row_num_per_page":"=@rows"}});
    let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    let rd = rpx_core::open_bytes(&rpx_core::to_bytes(&rd).unwrap()).unwrap();
    let mut params = rpx_engine::ParamTable::new();
    params.insert("rows", rpx_engine::value::Value::Int(2));
    let g = rpx_engine::calculate_with(&rd, &[], &params).unwrap();
    let pg = paginate_with_setup(&g, &g.print_setup).unwrap();
    assert_eq!(
        pg.pages
            .iter()
            .map(|p| p.row_indices.clone())
            .collect::<Vec<_>>(),
        vec![vec![1, 2], vec![3, 4], vec![5]]
    );
    params.insert("rows", rpx_engine::value::Value::Int(0));
    assert!(rpx_engine::calculate_with(&rd, &[], &params)
        .unwrap_err()
        .to_string()
        .contains("正整数"));
}

#[test]
fn conditional_group_break_occurs_after_last_detail_and_only_in_paper_mode() {
    use rpx_engine::{expr::DataSet, value::Value};
    let doc = json!({"grid":{"rows":1,"cols":2},"cells":[
        {"row":1,"col":1,"expr":"ds1.group(team)"},
        {"row":1,"col":2,"expr":"ds1.select(amount)"}
    ], "group_breaks":[{"cell":"A1","when":"=A1=='A'"}]});
    let ds = DataSet::new(
        "ds1",
        &["team", "amount"],
        [("A", 1), ("A", 2), ("B", 3), ("B", 4), ("C", 5)]
            .map(|(t, n)| vec![Value::Text(t.into()), Value::Int(n)])
            .to_vec(),
    );
    let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    let g = rpx_engine::calculate(&rd, &[ds]).unwrap();
    assert_eq!(g.row_break_page, vec![false, true, false, false, false]);
    let pg = paginate_with_setup(&g, &g.print_setup).unwrap();
    assert_eq!(
        pg.pages
            .iter()
            .map(|p| p.row_indices.clone())
            .collect::<Vec<_>>(),
        vec![vec![1, 2], vec![3, 4, 5]]
    );
    let mut ps = g.print_setup.clone();
    ps.pager_style = 0;
    assert_eq!(paginate_with_setup(&g, &ps).unwrap().page_count, 1);
    ps.pager_style = 1;
    ps.row_num_per_page = 20;
    assert_eq!(paginate_with_setup(&g, &ps).unwrap().page_count, 1);
}
