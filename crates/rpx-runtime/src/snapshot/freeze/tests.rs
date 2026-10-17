use rpx_core::ReportDefine;
use rpx_engine::{
    arena::{CalcCell, CalcGrid, MergedCell},
    expr::ds::DataSet,
    pager::{paginate_with_setup, project_page, PageInfo, Pagination},
    value::Value,
};
use serde_json::{json, Value as J};

fn definition(doc: J) -> ReportDefine {
    crate::reportdef::build(&doc.to_string()).unwrap()
}

fn workbook(rd: &ReportDefine, grid: &CalcGrid) -> J {
    let before = grid.clone();
    let result =
        serde_json::from_str(&crate::snapshot::build(rd, grid, "freeze").unwrap()).unwrap();
    assert_eq!(grid, &before, "生成浏览冻结不能修改原始网格");
    result
}

fn frozen(rows: i32, cols: i32) -> J {
    json!({"xSplit":cols,"ySplit":rows,
        "startRow":if rows == 0 {-1} else {rows},
        "startColumn":if cols == 0 {-1} else {cols}})
}

fn band_grid(rows: &[i8], cols: &[i8]) -> (ReportDefine, CalcGrid) {
    let rd = definition(json!({"grid":{"rows":1,"cols":1}}));
    let mut grid = CalcGrid::new(rows.len() as i32, cols.len() as i32);
    grid.row_types = rows.to_vec();
    grid.col_types = cols.to_vec();
    grid.row_heights = vec![8.0; rows.len()];
    grid.col_widths = vec![25.0; cols.len()];
    (rd, grid)
}

fn assert_content(sheet: &J, rd: &ReportDefine, grid: &CalcGrid) {
    assert_eq!(sheet["cellData"], crate::snapshot::cell_data(rd, grid));
    assert_eq!(
        sheet["mergeData"],
        json!(crate::snapshot::geom::merge_data(grid).unwrap())
    );
    assert_eq!(sheet["rowData"], crate::snapshot::geom::row_data(grid));
    assert_eq!(sheet["columnData"], crate::snapshot::geom::col_data(grid));
}

#[test]
fn freezes_only_the_contiguous_leading_header_bands() {
    let (rd, grid) = band_grid(
        &[-93, -96, -95, -95, -91, -95, -90, -94, -92],
        &[-80, -80, -79, -80, -78],
    );
    let wb = workbook(&rd, &grid);
    let sheet = &wb["sheets"]["rpx-sheet-1"];
    assert_eq!(sheet["freeze"], frozen(4, 2));
    assert_content(sheet, &rd, &grid);
}

#[test]
fn without_a_leading_header_each_axis_has_the_native_unfrozen_sentinel() {
    for rows in [vec![-91, -95], vec![-90, -95], vec![-94, -96]] {
        let (rd, grid) = band_grid(&rows, &[-79, -80]);
        assert_eq!(
            workbook(&rd, &grid)["sheets"]["rpx-sheet-1"]["freeze"],
            frozen(0, 0)
        );
    }
    let (rd, mut grid) = band_grid(&[-95], &[-80]);
    grid.row_types.clear();
    grid.col_types.clear();
    assert_eq!(
        workbook(&rd, &grid)["sheets"]["rpx-sheet-1"]["freeze"],
        frozen(0, 0),
        "缺少区带信息时不能猜测表头"
    );
}

#[test]
fn supports_either_axis_and_header_only_grids_without_freezing_padding() {
    for (rows, cols, expected) in [
        (vec![-96, -95, -91], vec![-79], frozen(2, 0)),
        (vec![-91], vec![-80, -79], frozen(0, 1)),
        (vec![-93, -91], vec![-79], frozen(1, 0)),
        (vec![-95, -95], vec![-80, -80], frozen(2, 2)),
    ] {
        let (rd, grid) = band_grid(&rows, &cols);
        let wb = workbook(&rd, &grid);
        let sheet = &wb["sheets"]["rpx-sheet-1"];
        assert_eq!(sheet["freeze"], expected);
        assert!(sheet["rowCount"].as_i64().unwrap() > grid.rows as i64);
        assert!(sheet["columnCount"].as_i64().unwrap() > grid.cols as i64);
    }
}

#[test]
fn header_metadata_never_freezes_past_the_calculated_grid_boundary() {
    let (rd, mut grid) = band_grid(&[-95, -95], &[-80, -80]);
    grid.rows = 1;
    grid.cols = 1;
    assert_eq!(
        workbook(&rd, &grid)["sheets"]["rpx-sheet-1"]["freeze"],
        frozen(1, 1)
    );
    grid.rows = 0;
    grid.cols = 0;
    assert_eq!(
        workbook(&rd, &grid)["sheets"]["rpx-sheet-1"]["freeze"],
        frozen(0, 0)
    );
}

#[test]
fn hidden_headers_keep_the_same_indices_as_cells_and_merges() {
    let (rd, mut grid) = band_grid(&[-96, -95, -95, -91], &[-80, -80, -79]);
    grid.row_visible = vec![false, true, false, true];
    grid.col_visible = vec![false, true, true];
    grid.cells
        .push(CalcCell::bare(2, 1, Value::Text("header".into())));
    grid.cells.push(CalcCell::bare(4, 3, Value::Int(42)));
    for c in 1..=2 {
        grid.merged
            .push(MergedCell::new(1, 2, c, Value::Text("header".into()), 1, 2));
    }
    let wb = workbook(&rd, &grid);
    let sheet = &wb["sheets"]["rpx-sheet-1"];
    assert_eq!(sheet["freeze"], frozen(3, 2));
    assert_eq!(sheet["rowData"]["0"]["hd"], 1);
    assert_eq!(sheet["rowData"]["2"]["hd"], 1);
    assert_eq!(sheet["columnData"]["0"]["hd"], 1);
    assert_eq!(sheet["cellData"]["3"]["2"]["v"], 42);
    assert_eq!(
        sheet["mergeData"],
        json!([
            {"startRow":1,"endRow":1,"startColumn":0,"endColumn":1}
        ])
    );
    assert_content(sheet, &rd, &grid);
}

#[test]
fn hidden_data_does_not_connect_disjoint_header_bands() {
    let (rd, mut grid) = band_grid(&[-95, -91, -95], &[-80, -79, -80]);
    grid.row_visible = vec![true, false, true];
    grid.col_visible = vec![true, false, true];
    let wb = workbook(&rd, &grid);
    let sheet = &wb["sheets"]["rpx-sheet-1"];
    assert_eq!(sheet["freeze"], frozen(1, 1));
    assert_eq!(sheet["rowData"]["1"]["hd"], 1);
    assert_eq!(sheet["columnData"]["1"]["hd"], 1);
}

#[test]
fn uses_expanded_row_and_column_headers_instead_of_template_counts() {
    let rd = definition(json!({
        "grid":{"rows":3,"cols":3},
        "cells":[
            {"row":1,"col":1,"expr":"vertical.select(label)","extend_mode":"vertical"},
            {"row":2,"col":2,"expr":"horizontal.select(label)","extend_mode":"horizontal"},
            {"row":3,"col":3,"text":"data"}
        ],
        "row_props":[{"row":1,"type":"table_header"}],
        "col_props":[{"col":1,"type":"left_header"},{"col":2,"type":"left_header"}],
        "print_setup":{"pager_style":"none"}
    }));
    let datasets = [
        DataSet::new(
            "vertical",
            &["label"],
            vec![
                vec![Value::Int(1)],
                vec![Value::Int(2)],
                vec![Value::Int(3)],
            ],
        ),
        DataSet::new(
            "horizontal",
            &["label"],
            vec![vec![Value::Text("A".into())], vec![Value::Text("B".into())]],
        ),
    ];
    let mut grid = rpx_engine::calculate(&rd, &datasets).unwrap();
    assert_eq!((grid.rows, grid.cols), (5, 4));
    assert_eq!(grid.row_types, [-95, -95, -95, -91, -91]);
    assert_eq!(grid.col_types, [-80, -80, -80, -79]);
    grid.pagination = Some(paginate_with_setup(&grid, &rd.print_setup).unwrap());
    let wb = workbook(&rd, &grid);
    let sheet = &wb["sheets"]["rpx-sheet-1"];
    assert_eq!(sheet["freeze"], frozen(3, 3));
    assert_eq!(sheet["cellData"]["4"]["3"]["v"], "data");
    assert_content(sheet, &rd, &grid);
}

#[test]
fn merged_titles_never_turn_data_columns_into_frozen_left_headers() {
    let rd = definition(json!({
        "grid":{"rows":3,"cols":4},
        "cells":[
            {"row":1,"col":1,"text":"Title","merge":{"rows":1,"cols":4}},
            {"row":2,"col":1,"text":"Year"},{"row":2,"col":2,"text":"Month"},
            {"row":2,"col":3,"text":"East"},{"row":2,"col":4,"text":"West"},
            {"row":3,"col":1,"text":"2026"},{"row":3,"col":2,"text":"10"},
            {"row":3,"col":3,"expr":"42"},{"row":3,"col":4,"expr":"84"}
        ],
        "row_props":[{"row":1,"type":"title"},{"row":2,"type":"table_header"}],
        "col_props":[{"col":1,"type":"left_header"},{"col":2,"type":"left_header"}],
        "print_setup":{"pager_style":"none"}
    }));
    assert_eq!(
        rd.col_cells.iter().map(|c| c.col_type).collect::<Vec<_>>(),
        [-80, -80, -79, -79]
    );
    // 多次计算覆盖 HashMap 不同遍历次序：合并锚点别名不能成为列类型来源。
    for _ in 0..32 {
        let grid = rpx_engine::calculate(&rd, &[]).unwrap();
        let wb = workbook(&rd, &grid);
        let sheet = &wb["sheets"]["rpx-sheet-1"];
        assert_eq!(
            sheet["freeze"],
            frozen(2, 2),
            "计算态列类型: {:?}",
            grid.col_types
        );
        assert_eq!(sheet["cellData"]["2"]["2"]["v"], 42);
        assert_eq!(sheet["cellData"]["2"]["3"]["v"], 84);
        assert_eq!(
            sheet["mergeData"],
            json!([
                {"startRow":0,"endRow":0,"startColumn":0,"endColumn":3}
            ])
        );
    }
}

fn page(number: i32, rows: &[i32], cols: &[i32]) -> PageInfo {
    PageInfo {
        panels: Vec::new(),
        page: number,
        rows: rows.len() as i32,
        row_range: (*rows.first().unwrap(), *rows.last().unwrap()),
        col_range: (*cols.first().unwrap(), *cols.last().unwrap()),
        row_indices: rows.to_vec(),
        col_indices: cols.to_vec(),
        col1_values: Vec::new(),
    }
}

#[test]
fn each_page_freezes_its_own_repeated_headers_with_local_indices() {
    let (rd, mut grid) = band_grid(&[-96, -95, -95, -91, -91, -91], &[-80, -80, -79, -79]);
    grid.row_visible = vec![true, false, true, true, true, true];
    grid.col_visible = vec![false, true, true, true];
    for r in 1..=grid.rows {
        for c in 1..=grid.cols {
            grid.cells
                .push(CalcCell::bare(r, c, Value::Int(r * 10 + c)));
        }
    }
    for r in 2..=3 {
        grid.merged
            .push(MergedCell::new(1, r, 2, Value::Int(22), 2, 1));
    }
    grid.pagination = Some(Pagination {
        page_count: 3,
        pages: vec![
            page(1, &[1, 2, 3, 4], &[1, 2, 3]),
            page(2, &[2, 3, 5], &[1, 2, 4]),
            page(3, &[6], &[4]),
        ],
        ..Default::default()
    });
    let wb = workbook(&rd, &grid);
    for (index, expected) in [frozen(3, 2), frozen(2, 2), frozen(0, 0)]
        .into_iter()
        .enumerate()
    {
        let sheet = &wb["sheets"][format!("rpx-sheet-{}", index + 1)];
        assert_eq!(sheet["freeze"], expected);
        let projected = project_page(&grid, &grid.pagination.as_ref().unwrap().pages[index]);
        assert_content(sheet, &rd, &projected);
    }
    let second = &wb["sheets"]["rpx-sheet-2"];
    assert_eq!(second["rowData"]["0"]["hd"], 1);
    assert_eq!(second["columnData"]["0"]["hd"], 1);
    assert_eq!(second["cellData"]["2"]["2"]["v"], 54);
    assert_eq!(
        second["mergeData"],
        json!([
            {"startRow":0,"endRow":1,"startColumn":1,"endColumn":1}
        ])
    );
    assert_eq!(wb["pagination"], crate::snapshot::pagination_json(&grid));
}

#[test]
fn a_single_projected_page_uses_its_local_headers_too() {
    let (rd, mut grid) = band_grid(&[-96, -95, -91, -91], &[-80, -79, -79]);
    grid.cells.push(CalcCell::bare(4, 3, Value::Int(43)));
    grid.pagination = Some(Pagination {
        page_count: 1,
        pages: vec![page(1, &[2, 4], &[1, 3])],
        ..Default::default()
    });
    let wb = workbook(&rd, &grid);
    let sheet = &wb["sheets"]["rpx-sheet-1"];
    assert_eq!(sheet["freeze"], frozen(1, 1));
    assert_eq!(sheet["cellData"]["1"]["1"]["v"], 43);
    let projected = project_page(&grid, &grid.pagination.as_ref().unwrap().pages[0]);
    assert_content(sheet, &rd, &projected);
}

#[test]
fn configured_two_axis_paging_repeats_headers_without_changing_data_or_merges() {
    let mut cells = Vec::new();
    for r in 1..=8 {
        for c in 1..=5 {
            if r == 4 && c == 1 {
                continue;
            }
            let mut cell = json!({"row":r,"col":c,"text":format!("R{r}C{c}")});
            if r == 3 && c == 1 {
                cell["merge"] = json!({"rows":2,"cols":1});
            }
            cells.push(cell);
        }
    }
    let rd = definition(json!({
        "grid":{"rows":8,"cols":5}, "cells":cells,
        "row_props":[
            {"row":1,"type":"page_header"},{"row":2,"type":"title"},
            {"row":3,"type":"table_header"},{"row":4,"type":"table_header"}
        ],
        "col_props":[
            {"col":1,"type":"left_header","width":10},
            {"col":2,"type":"left_header","width":10},
            {"col":3,"width":10},{"col":4,"width":10},{"col":5,"width":10}
        ],
        "printing":{"header_y":"first","title_y":"first"},
        "print_setup":{"pager_style":"rows","row_num_per_page":2,
            "paper_width":35,"paper_height":200,
            "left_margin":0,"right_margin":0,"top_margin":0,"bottom_margin":0}
    }));
    let mut grid = rpx_engine::calculate(&rd, &[]).unwrap();
    grid.pagination = Some(paginate_with_setup(&grid, &rd.print_setup).unwrap());
    let pg = grid.pagination.as_ref().unwrap();
    assert_eq!((pg.x_pages, pg.y_pages, pg.page_count), (3, 2, 6));
    let wb = workbook(&rd, &grid);
    for (index, page) in pg.pages.iter().enumerate() {
        let sheet = &wb["sheets"][format!("rpx-sheet-{}", page.page)];
        assert_eq!(sheet["freeze"], frozen(if index < 3 { 4 } else { 2 }, 2));
        assert_content(sheet, &rd, &project_page(&grid, page));
    }
    let last = &wb["sheets"]["rpx-sheet-6"];
    assert_eq!(last["cellData"]["2"]["2"]["v"], "R7C5");
    assert_eq!(last["cellData"]["3"]["2"]["v"], "R8C5");
    assert_eq!(
        last["mergeData"],
        json!([
            {"startRow":0,"endRow":1,"startColumn":0,"endColumn":0}
        ])
    );
    assert_eq!(wb["pagination"]["page_count"], 6);
}
