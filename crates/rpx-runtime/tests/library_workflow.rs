use rpx_runtime::{definition, files, import, library, reportdef, run};
use serde_json::json;
use std::{fs, path::PathBuf};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("rpx-library-{}", files::unique()));
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[tokio::test]
async fn source_snapshot_and_failed_attempt_preserve_a_success() {
    let temp = Temp::new();
    let root = &temp.0;
    library::init(root, true).unwrap();
    let original = library::show(root, "grouped-sales").unwrap();
    let rev = original["revision"].as_str().unwrap();
    let success = library::execute(root, "grouped-sales", rev, &run::Options::default())
        .await
        .unwrap();
    let result = success["result"].as_str().unwrap();
    assert!(std::path::Path::new(result).is_file());
    let missing = run::Options {
        data_dir: Some(root.join("nonexistent")),
        ..Default::default()
    };
    let error = library::execute(root, "grouped-sales", rev, &missing)
        .await
        .unwrap_err();
    assert_eq!(error.exit, 4);
    let state = library::show(root, "grouped-sales").unwrap();
    assert_eq!(state["display_run_id"], success["run"]["run_id"]);
    assert_eq!(state["latest_attempt"]["status"], "failed");
    let latest = state["latest_attempt"]["run_id"].as_str().unwrap();
    assert!(library::artifact(root, "grouped-sales", latest, "result.json").is_err());
    let opts = run::Options::default();
    let source = run::capture(&root.join("grouped-sales/template.json"), &opts).unwrap();
    let old = source.template.clone();
    let out = root.join("standalone-run");
    let record = run::start(&source, &out).unwrap();
    let patch = json!([{"op":"replace","path":"/cells/0/text","value":"changed while running"}]);
    library::patch(root, "grouped-sales", rev, &patch, false).unwrap();
    // Execute from captured bytes after the managed file has changed.
    let source = run::Source {
        data_dir: root.join("grouped-sales/data"),
        ..source
    };
    run::execute(source, &opts, &out, record).await.unwrap();
    assert_eq!(
        files::read_json(&out.join("source/template.json")).unwrap(),
        old
    );
    assert_ne!(
        library::show(root, "grouped-sales").unwrap()["revision"],
        rev
    );
}
#[test]
fn import_preserves_typed_values_and_rejects_unmapped_semantics() {
    let j = json!({"grid":{"rows":1,"cols":5},"cells":[
     {"row":1,"col":1,"literal":{"type":"long","value":"9007199254740993"}},
     {"row":1,"col":2,"literal":{"type":"decimal","value":"-100.00"}},
     {"row":1,"col":3,"literal":{"type":"bool","value":true}},
     {"row":1,"col":4,"literal":{"type":"null","value":null},"disp_value":"仅显示值"},
     {"row":1,"col":5,"literal":{"type":"text","value":""}}
    ]});
    let rd = reportdef::build(&j.to_string()).unwrap();
    let bytes = rpx_core::to_bytes(&rd).unwrap();
    let converted = import::convert(&bytes).unwrap();
    assert_eq!(definition::parse(&converted).unwrap().0, rd);
    let mut bad = rd;
    bad.store[0].hyperlink = Some("https://example.com".into());
    let error = import::convert(&rpx_core::to_bytes(&bad).unwrap()).unwrap_err();
    assert_eq!(error.diagnostic["code"], "RPX_IMPORT_UNSUPPORTED");
    assert!(error.diagnostic["json_pointer"]
        .as_str()
        .unwrap()
        .contains("A1"));
}
#[test]
fn simultaneous_master_and_dependent_are_validated_as_a_final_definition() {
    let original = json!({"grid":{"rows":6,"cols":4},"cells":[]});
    let patch = json!([{"op":"add","path":"/cells/-","value":{"row":6,"col":3,"expr":"D6*10","left_head":"D6"}},
  {"op":"add","path":"/cells/-","value":{"row":6,"col":4,"expr":"to(1,3)","extend_mode":"vertical"}}]);
    let changed = definition::patch(&original, &patch).unwrap();
    assert_eq!(changed["cells"][0]["left_head"], "D6");
    assert!(definition::patch(
        &original,
        &json!([{"op":"add","path":"/unexpected","value":1}])
    )
    .is_err());
}

#[test]
fn all_bundled_examples_survive_explicit_rpx_to_json_conversion() {
    let temp = Temp::new();
    fs::create_dir(&temp.0).unwrap();
    for example in rpx_runtime::catalog::examples() {
        let dest = temp.0.join(example.id);
        rpx_runtime::catalog::write_package(example.id, &dest, false).unwrap();
        let source = files::read_json(&dest.join("template.json")).unwrap();
        let rd = definition::parse(&source).unwrap().0;
        let imported = import::convert(&rpx_core::to_bytes(&rd).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e:?}", example.id));
        let datasets = rd
            .datasets
            .iter()
            .filter_map(|d| d.file.as_ref())
            .map(|f| {
                let name = f.file_name.as_ref().unwrap();
                (name.clone(), fs::read(dest.join(name)).unwrap())
            })
            .collect();
        let mut request = rpx_runtime::form::RenderRequest {
            report: serde_json::to_vec(&source).unwrap(),
            report_name: example.id.into(),
            datasets,
            params: Default::default(),
        };
        let expected = rpx_runtime::execution::render_request(&request).unwrap();
        request.report = serde_json::to_vec(&imported).unwrap();
        assert_eq!(
            rpx_runtime::execution::render_request(&request).unwrap(),
            expected,
            "{}",
            example.id
        );
    }
}

#[tokio::test]
async fn requested_export_failure_keeps_successful_artifacts() {
    let temp = Temp::new();
    library::init(&temp.0, true).unwrap();
    let detail = library::show(&temp.0, "images").unwrap();
    let revision = detail["revision"].as_str().unwrap();
    // This template has a printable background unsupported by Excel. CSV is staged first.
    let run = library::execute(
        &temp.0,
        "images",
        revision,
        &run::Options {
            exports: vec!["csv".into(), "print-xlsx".into()],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(run["run"]["status"], "succeeded");
    assert_eq!(run["run"]["diagnostics"][0]["code"], "EXPORT_FAILED");
    assert!(run["run"]["artifacts"]["report.csv"].is_string());
    assert!(run["run"]["artifacts"]["result.json"].is_string());
    assert!(run["run"]["artifacts"].get("report-print.xlsx").is_none());
    let state = library::show(&temp.0, "images").unwrap();
    assert_eq!(state["display_run_id"], run["run"]["run_id"]);
    assert_eq!(state["latest_attempt"]["status"], "succeeded");
    let run_id = run["run"]["run_id"].as_str().unwrap();
    assert!(library::artifact(&temp.0, "images", run_id, "report.csv").is_ok());
    assert!(library::artifact(&temp.0, "images", run_id, "report-print.xlsx").is_err());
}

#[test]
fn offline_checks_locate_cycles_and_reject_unrepresentable_grid_numbers() {
    let cycle = json!({"grid":{"rows":1,"cols":2},"cells":[
        {"row":1,"col":1,"expr":"B1+1"},{"row":1,"col":2,"expr":"A1+1"}
    ]});
    let error = definition::validate(&cycle, None).unwrap_err();
    assert_eq!(error.diagnostic["code"], "CIRCULAR_REFERENCE");
    assert_eq!(error.diagnostic["json_pointer"], "/cells/1/expr");
    for invalid in [
        json!({"grid":{"rows":18446744073709551615u64,"cols":1}}),
        json!({"grid":{"rows":1,"cols":1},"row_props":[{"row":1,"height":1e100}]}),
    ] {
        assert_eq!(definition::validate(&invalid, None).unwrap_err().exit, 3);
    }
}

#[tokio::test]
async fn typed_result_preserves_large_integers_decimal_scales_and_display_only() {
    let temp = Temp::new();
    fs::create_dir(&temp.0).unwrap();
    let literals = json!({"grid":{"rows":1,"cols":5},"cells":[
        {"row":1,"col":1,"literal":{"type":"long","value":"9007199254740993"}},
        {"row":1,"col":2,"literal":{"type":"decimal","value":"-100.00"}},
        {"row":1,"col":3,"literal":{"type":"decimal","value":"1.2300E+5"}},
        {"row":1,"col":4,"literal":{"type":"null","value":null},"disp_value":"仅显示值"},
        {"row":1,"col":5,"expr":"\"\""}
    ]});
    let file = temp.0.join("template.json");
    files::create(&file, &files::json_bytes(&literals).unwrap()).unwrap();
    let opts = run::Options::default();
    let source = run::capture(&file, &opts).unwrap();
    let out = temp.0.join("run");
    let record = run::start(&source, &out).unwrap();
    run::execute(source, &opts, &out, record).await.unwrap();
    let result = files::read_json(&out.join("artifacts/result.json")).unwrap();
    let cells = result["cells"].as_array().unwrap();
    assert_eq!(
        cells[0]["value"],
        json!({"type":"long","value":"9007199254740993"})
    );
    assert_eq!(
        cells[1]["value"],
        json!({"type":"decimal","value":"-100.00"})
    );
    assert_eq!(cells[2]["value"]["value"], "1.2300E+5");
    assert_eq!(cells[3]["value"], json!({"type":"null","value":null}));
    assert_eq!(cells[3]["display_override"], "仅显示值");
    assert_eq!(cells[4]["value"], json!({"type":"string","value":""}));
    assert!(cells[4].get("display_override").is_none());
    // RPX import must preserve negative scale rather than expanding or rounding its decimal.
    let rd = definition::parse(&literals).unwrap().0;
    let imported = import::convert(&rpx_core::to_bytes(&rd).unwrap()).unwrap();
    assert_eq!(definition::parse(&imported).unwrap().0, rd);
}
