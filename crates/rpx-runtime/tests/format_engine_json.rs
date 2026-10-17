#![cfg(feature = "legacy-rpx")]
//! Phase 2 gate: the same engine fixtures must calculate from the committed
//! JSON definitions, not from their original `.rpx` bytes at test time.

use rpx_runtime::{format, parameters};
use serde_json::{json, Map, Value as J};
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn names() -> [&'static str; 23] {
    [
        "engine_sum",
        "engine_ext",
        "engine_expr",
        "engine_pager",
        "engine_merge",
        "engine_cycle",
        "engine_samemerge",
        "engine_disp",
        "engine_types",
        "engine_rowheight",
        "engine_head_top",
        "engine_head_left",
        "engine_avgrange",
        "engine_collate",
        "engine_listrange",
        "engine_param",
        "engine_gbk",
        "engine_emp",
        "engine_cross",
        "engine_cross_root",
        "engine_crossalign",
        "engine_crossalign_lit",
        "engine_crossalign_ds2",
    ]
}

fn params(expected: &J) -> J {
    let Some(list) = expected["params"].as_array() else {
        return json!({});
    };
    let mut out = Map::new();
    for item in list {
        if let (Some(name), Some(value)) = (item["name"].as_str(), item.get("value")) {
            out.insert(name.to_string(), value.clone());
        }
    }
    J::Object(out)
}

#[test]
fn engine_fixtures_calculate_from_json_projection() {
    let dir = fixtures_dir();
    let mut failures = Vec::new();
    let mut checked = 0;
    for name in names() {
        let json_path = dir.join("format/engine").join(format!("{name}.json"));
        let document: J = serde_json::from_slice(&std::fs::read(&json_path).unwrap())
            .unwrap_or_else(|e| panic!("{name}: fixture JSON: {e}"));
        let rd = format::json_to_model(&document)
            .unwrap_or_else(|e| panic!("{name}: json_to_model: {e}"));
        let expected: J = serde_json::from_slice(
            &std::fs::read(dir.join(format!("{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let params = parameters::parse(&params(&expected)).unwrap();
        let datasets = match rpx_engine::dataset::load_datasets(&dir, &rd) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{name}: load_datasets: {e}"));
                continue;
            }
        };
        match rpx_engine::calculate_with(&rd, &datasets, &params) {
            Ok(grid) => {
                if expected["calculated"]["error"].is_object() {
                    failures.push(format!("{name}: expected error, got grid"));
                    continue;
                }
                let actual: J = serde_json::from_str(&rpx_engine::emit::render_document(&grid))
                    .unwrap_or_else(|e| panic!("{name}: emit JSON: {e}"));
                for section in ["calculated", "pagination", "merged"] {
                    if actual[section] != expected[section] {
                        failures.push(format!("{name}: {section} differs"));
                        break;
                    }
                }
                checked += 1;
            }
            Err(e) => {
                if expected["calculated"]["error"].is_object() {
                    checked += 1;
                } else {
                    failures.push(format!("{name}: unexpected error: {e}"));
                }
            }
        }
    }
    assert_eq!(checked, names().len(), "checked {checked}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn edited_preserve_json_keeps_unmodeled_cell_fields() {
    let dir = fixtures_dir();
    let path = dir.join("format/engine/engine_merge.json");
    let mut doc: J = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let cell = doc["cells"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["row"] == 2 && c["col"] == 1)
        .expect("A2");
    cell["font_size"] = json!(13);
    let rebuilt = format::json_to_rpx(
        &doc,
        format::Options {
            mode: format::Mode::PreserveLegacy,
            allow_structural: false,
        },
    )
    .unwrap();
    let rd = rpx_core::open_bytes(&rebuilt).unwrap();
    let a2 = rd.cell_at(2, 1).unwrap();
    let a3 = rd.cell_at(3, 1).unwrap();
    assert_eq!(a2.font_size, 13);
    assert_eq!(a2.extend_mode, 18);
    assert_eq!(
        a3.extend_mode, 16,
        "independent merge continuation preserved"
    );
    assert!(a3.merged_area.is_none());
    assert!(a2.exp_map.is_some(), "unmodeled exp_map preserved");
}
