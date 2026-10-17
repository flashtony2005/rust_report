//! Phase 1 migration corpus: every product `.rpx` must survive the v1
//! Preserve path, and the raw snapshot must round-trip byte-for-byte.

use rpx_runtime::format::{self, Mode, Options};
use std::path::PathBuf;

fn fixtures() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("rpx"))
        .collect();
    out.sort();
    out
}

#[test]
fn all_product_rpx_files_preserve_roundtrip() {
    let files = fixtures();
    assert_eq!(
        files.len(),
        40,
        "fixture count changed; update this baseline"
    );
    let mut failures = Vec::new();
    let mut warnings = 0;
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy();
        let converted = match format::rpx_to_json(
            &bytes,
            Options {
                mode: Mode::PreserveLegacy,
            },
        ) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{name}: to-json: {e}"));
                continue;
            }
        };
        warnings += converted.warnings.len();
        assert_eq!(converted.document["format"], "rpx-report", "{name}");
        assert_eq!(converted.document["format_version"], 1, "{name}");
        assert!(
            converted.document["extensions"]["rpx.legacy"].is_object(),
            "{name}: missing legacy snapshot"
        );
        let modeled = format::json_to_model(&converted.document)
            .unwrap_or_else(|e| panic!("{name}: modeled JSON must parse: {e}"));
        let original = rpx_core::open_bytes(&bytes).unwrap();
        assert_eq!(modeled.rows, original.rows, "{name}: rows");
        assert_eq!(modeled.cols, original.cols, "{name}: cols");
        assert_eq!(
            modeled.datasets.len(),
            original.datasets.len(),
            "{name}: datasets"
        );
        assert_eq!(
            modeled.params.len(),
            original.params.len(),
            "{name}: params"
        );
        match format::json_to_rpx(
            &converted.document,
            Options {
                mode: Mode::PreserveLegacy,
            },
        ) {
            Ok(rebuilt) if rebuilt == bytes => {}
            Ok(_) => failures.push(format!("{name}: bytes changed")),
            Err(e) => failures.push(format!("{name}: to-rpx: {e}")),
        }
        match format::verify(
            &bytes,
            Options {
                mode: Mode::PreserveLegacy,
            },
        ) {
            Ok(report) if report.model_equal && report.bytes_equal => {}
            Ok(report) => failures.push(format!("{name}: verify {report:?}")),
            Err(e) => failures.push(format!("{name}: verify error: {e}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures (warnings={warnings}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
