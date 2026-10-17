//! Phase 1/2 bridge: the same engine fixtures must calculate from the JSON
//! projection, not only from their original `.rpx` bytes.

use rpx_runtime::{
    format::{self, Mode, Options},
    parameters,
};
use serde_json::{json, Map, Value as J};
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn names() -> [&'static str; 1] {
    // Phase 1 的 modeled JSON 只覆盖 v1 可表达字段；这个夹具没有会改变计算的
    // 未建模字段，因此可以做完整的“JSON 投影 → 引擎 → 逐段对拍”。
    // 其余 22 个夹具的逐格对拍要等 Phase 2 的字段级 legacy 覆盖。
    ["engine_types"]
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
fn engine_types_calculates_from_json_projection() {
    let dir = fixtures_dir();
    let mut failures = Vec::new();
    let mut checked = 0;
    for name in names() {
        let bytes = std::fs::read(dir.join(format!("{name}.rpx"))).unwrap();
        let converted = format::rpx_to_json(
            &bytes,
            Options {
                mode: Mode::PreserveLegacy,
            },
        )
        .unwrap_or_else(|e| panic!("{name}: to-json: {e}"));
        let rd = format::json_to_model(&converted.document)
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
