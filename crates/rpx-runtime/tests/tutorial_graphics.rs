#![cfg(feature = "legacy-rpx")]
use rpx_engine::{expr::DataSet, graphics::GraphicContent, value::Value};
use serde_json::{json, Value as J};

fn definition() -> J {
    json!({"grid":{"rows":1,"cols":4},"cells":[
        {"row":1,"col":1,"expr":"ds1.group(team)"},
        {"row":1,"col":2,"expr":"ds1.select(item)"},
        {"row":1,"col":3,"expr":"ds1.amount"},
        {"row":1,"col":4,"left_head":"A1","data_type":"chart","chart":{
            "type":"column","title":"=A1","categories":"=B1{}","series":[{"name":"金额","values":"=C1{}"}],"width":320,"height":180
        }}
    ],"col_props":[{"col":2,"visible":false},{"col":3,"visible":false}]})
}
fn data() -> DataSet {
    DataSet::new(
        "ds1",
        &["team", "item", "amount"],
        [
            ("A", "a", 10),
            ("A", "b", 20),
            ("B", "c", 30),
            ("B", "d", 40),
        ]
        .map(|(t, i, n)| vec![Value::Text(t.into()), Value::Text(i.into()), Value::Int(n)])
        .to_vec(),
    )
}

#[test]
fn charts_follow_groups_and_bind_hidden_cells_after_rpx_roundtrip() {
    let rd = rpx_runtime::reportdef::build(&definition().to_string()).unwrap();
    let rd = rpx_core::open_bytes(&rpx_core::to_bytes(&rd).unwrap()).unwrap();
    let g = rpx_engine::calculate(&rd, &[data()]).unwrap();
    assert_eq!(g.graphics.len(), 2);
    let charts: Vec<_> = g
        .graphics
        .iter()
        .map(|g| match &g.content {
            GraphicContent::Chart(c) => c,
            _ => panic!("不是统计图"),
        })
        .collect();
    assert_eq!(
        charts.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(),
        ["A", "B"]
    );
    assert_eq!(
        charts[0].categories,
        [Value::Text("a".into()), Value::Text("b".into())]
    );
    assert_eq!(charts[0].series[0].values, [Value::Int(10), Value::Int(20)]);
    assert_eq!(charts[1].series[0].values, [Value::Int(30), Value::Int(40)]);
    let text = rpx_runtime::snapshot::build(&rd, &g, "groups").unwrap();
    let wb: J = serde_json::from_str(&text).unwrap();
    assert_eq!(
        wb["sheets"]["rpx-sheet-1"]["rpxGraphics"][1]["content"]["series"][0]["values"],
        json!([30, 40])
    );
    let bytes = rpx_export::to_print_xlsx(&g, &rd.print_setup).unwrap();
    use calamine::{Reader, Xlsx};
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    use std::io::Read;
    let names = archive.file_names().map(str::to_string).collect::<Vec<_>>();
    assert_eq!(
        names
            .iter()
            .filter(|n| n.starts_with("xl/charts/chart") && n.ends_with(".xml"))
            .count(),
        2
    );
    let mut chart_xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut chart_xml)
        .unwrap();
    assert!(chart_xml.contains("_chart_data"));
    assert!(chart_xml.contains("barChart"));
    let mut book = Xlsx::new(std::io::Cursor::new(&bytes)).unwrap();
    let hidden = book.worksheet_range("_chart_data").unwrap();
    assert_eq!(hidden.get_value((0, 1)), Some(&calamine::Data::Float(10.0)));
    assert_eq!(hidden.get_value((3, 1)), Some(&calamine::Data::Float(30.0)));
}

#[test]
fn all_twenty_eight_chart_bindings_survive_the_definition_and_rpx_chain() {
    use rpx_engine::graphics::ChartKind;
    assert_eq!(ChartKind::ALL.len(), 28);
    for kind in ChartKind::ALL {
        let mut chart = json!({"type":kind.name(),"title":"演示","categories":"=to(1,2)","series":[{"name":"下界","values":"=to(1,2)"},{"name":"上界","values":"=to(3,4)"}],
            "appearance":{"colors":["#123456","#abcdef"],"legend":"right","margins":[20,30,40,50],"pie_rotation":30,"pie_depth":12}});
        if kind.intervals() {
            chart.as_object_mut().unwrap().remove("categories");
            chart.as_object_mut().unwrap().remove("series");
            chart["tasks"] = json!({"categories":"=to(1,2)","starts":"=to(1000,1001)","ends":"=to(2000,2001)","states":"=to(1,2)"});
        }
        if matches!(
            kind,
            ChartKind::Pie | ChartKind::Pie3d | ChartKind::Gauge | ChartKind::Gauge3d
        ) {
            chart["series"] = json!([{"name":"金额","values":"=to(1,1)"}]);
            chart["categories"] = json!("=to(1,1)")
        }
        let doc = json!({"grid":{"rows":1,"cols":1},"cells":[{"row":1,"col":1,"data_type":"chart","chart":chart}]});
        let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
        let rd = rpx_core::open_bytes(&rpx_core::to_bytes(&rd).unwrap()).unwrap();
        let g = rpx_engine::calculate(&rd, &[]).unwrap();
        let GraphicContent::Chart(c) = &g.graphics[0].content else {
            panic!("chart")
        };
        assert_eq!(c.kind, *kind);
        assert_eq!(c.appearance.colors, ["#123456", "#abcdef"]);
        let snapshot: J =
            serde_json::from_str(&rpx_runtime::snapshot::build(&rd, &g, "all").unwrap()).unwrap();
        assert_eq!(
            snapshot["sheets"]["rpx-sheet-1"]["rpxGraphics"][0]["content"]["type"],
            kind.name()
        );
    }
}

#[test]
fn javascript_templates_receive_typed_parameters_and_fixed_frame_dimensions() {
    let doc = json!({"grid":{"rows":1,"cols":1},"cells":[{"row":1,"col":1,"data_type":"echarts_js","echarts":{"template":"return {title:{text:${caption}},series:[{type:'bar',data:${values}}],width:${width}};","params":{"caption":"='部门'","values":"=to(1,3)"},"width":300,"height":180}}]});
    let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    let g = rpx_engine::calculate(&rd, &[]).unwrap();
    let GraphicContent::EchartsScript(source) = &g.graphics[0].content else {
        panic!("script")
    };
    assert!(source.contains("[1,2,3]"));
    assert!(source.contains("width:300"));
    assert!(!source.contains("${"));
}

#[test]
fn invalid_dynamic_images_and_option_templates_are_input_errors() {
    for cell in [
        json!({"row":1,"col":1,"data_type":"image","image":{"source":"='data:image/png;base64,bad'"}}),
        json!({"row":1,"col":1,"data_type":"echarts","echarts":{"template":"[]"}}),
    ] {
        let req = rpx_runtime::form::RenderRequest {
            report: json!({"grid":{"rows":1,"cols":1},"cells":[cell]})
                .to_string()
                .into_bytes(),
            report_name: "invalid".into(),
            datasets: Default::default(),
            params: Default::default(),
        };
        let error = rpx_runtime::execution::render_request(&req).unwrap_err();
        assert!(!format!("{error}").is_empty());
    }
}

#[test]
fn images_and_page_backgrounds_survive_projection_and_real_image_export() {
    let png="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jZ1kAAAAASUVORK5CYII=";
    let doc = json!({"grid":{"rows":1,"cols":2},"background_image":png,"cells":[{"row":1,"col":1,"expr":"to(1,3)"},{"row":1,"col":2,"data_type":"image","image":{"source":png,"width":30,"height":25}}],"print_setup":{"pager_style":"rows","row_num_per_page":1}});
    let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    let rd = rpx_core::open_bytes(&rpx_core::to_bytes(&rd).unwrap()).unwrap();
    let g = rpx_engine::calculate(&rd, &[]).unwrap();
    assert_eq!(g.graphics.len(), 3);
    let p = rpx_engine::pager::paginate_with_setup(&g, &g.print_setup).unwrap();
    let second = rpx_engine::pager::project_page(&g, &p.pages[1]);
    assert_eq!(second.page_background.as_deref(), Some(png));
    assert_eq!(second.graphics[0].row, 1);
    let bytes = rpx_export::to_print_xlsx(&g, &g.print_setup).unwrap();
    let zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert!(zip
        .file_names()
        .any(|n| n.starts_with("xl/media/") && n.ends_with(".png")));
    let mut ps = g.print_setup.clone();
    ps.back_graph_printed = true;
    assert!(rpx_export::to_print_xlsx(&g, &ps)
        .unwrap_err()
        .contains("背景"));
}

#[test]
fn chart_category_mismatch_fails_instead_of_dropping_data() {
    let mut doc = definition();
    doc["cells"][3]["chart"]["series"][0]["values"] = json!("=to(1,1)");
    let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    let error = rpx_engine::calculate(&rd, &[data()])
        .unwrap_err()
        .to_string();
    assert!(error.contains("分类轴有 2 项"), "{error}");
}

#[test]
fn negative_pie_data_and_reversed_numeric_tasks_are_rejected() {
    for (chart, message) in [
        (
            json!({"type":"pie","categories":"=1","series":[{"name":"金额","values":"=-1"}]}),
            "负数",
        ),
        (
            json!({"type":"gantt","tasks":{"categories":"=1","starts":"=2000","ends":"=1000"}}),
            "早于",
        ),
    ] {
        let doc = json!({"grid":{"rows":1,"cols":1},"cells":[{"row":1,"col":1,"data_type":"chart","chart":chart}]});
        let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
        let error = rpx_engine::calculate(&rd, &[]).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn echarts_template_uses_typed_single_pass_parameters() {
    let doc = json!({"grid":{"rows":1,"cols":2},"cells":[
        {"row":1,"col":1,"expr":"to(1,3)"},
        {"row":1,"col":2,"data_type":"echarts","echarts":{
            "template":"{\"title\":{\"text\":${title}},\"series\":[{\"type\":\"bar\",\"data\":${values}}]}",
            "params":{"title":"='literal ${not_a_macro}'","values":"=A1[`0]{}"}
        }}
    ]});
    let rd = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    // `${...}` in the value is report macro syntax before evaluation. Use a typed parameter
    // to exercise template substitution without report-macro expansion touching it.
    let mut params = rpx_engine::ParamTable::new();
    params.insert("caption", Value::Text("literal ${not_a_macro}".into()));
    let mut doc = doc;
    doc["cells"][1]["echarts"]["params"]["title"] = json!("=@caption");
    let rd2 = rpx_runtime::reportdef::build(&doc.to_string()).unwrap();
    let g = rpx_engine::calculate_with(&rd2, &[], &params).unwrap();
    let wb: J =
        serde_json::from_str(&rpx_runtime::snapshot::build(&rd2, &g, "template").unwrap()).unwrap();
    assert_eq!(
        wb["sheets"]["rpx-sheet-1"]["rpxGraphics"][0]["content"]["option"]["title"]["text"],
        "literal ${not_a_macro}"
    );
    assert!(rpx_engine::calculate(&rd, &[]).is_err());
}

#[test]
fn unsupported_chart_names_and_unprefixed_bindings_are_loud() {
    let mut doc = definition();
    doc["cells"][3]["chart"]["type"] = json!("fake_3d");
    assert!(rpx_runtime::reportdef::build(&doc.to_string())
        .unwrap_err()
        .to_string()
        .contains("chart.type"));
    let mut doc = definition();
    doc["cells"][3]["chart"]["categories"] = json!("B1{}");
    assert!(rpx_runtime::reportdef::build(&doc.to_string())
        .unwrap_err()
        .to_string()
        .contains("chart.categories"));
}
