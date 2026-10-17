//! JSON format v0/v1 marker and extension contract.
//!
//! The reader must keep accepting legacy (v0) definitions without a `format`
//! marker, while v1 documents are validated strictly.

use rpx_runtime::{
    definition,
    reportdef::{self, build, FORMAT, FORMAT_VERSION},
};
use serde_json::{json, Value as J};

fn v0() -> J {
    json!({"grid": {"rows": 1, "cols": 1}})
}

fn v1() -> J {
    json!({
        "format": "rpx-report",
        "format_version": 1,
        "schema": "rpx-report/v1",
        "grid": {"rows": 1, "cols": 1},
        "meta": {"title": "demo", "description": "format test", "tags": ["v1"]},
        "extensions": {"x-test": {"hello": "world"}, "rpx.legacy": {"format": "rpx-rpx-v1"}}
    })
}

fn build_j(value: &J) -> Result<(), String> {
    build(&value.to_string())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn legacy_v0_without_markers_still_builds() {
    build_j(&v0()).unwrap();
    assert_eq!(FORMAT, "rpx-report");
    assert_eq!(FORMAT_VERSION, 1);
}

#[test]
fn v1_markers_meta_and_extensions_build() {
    build_j(&v1()).unwrap();
}

#[test]
fn v1_without_deprecated_version_alias_is_accepted() {
    build_j(&v1()).unwrap();
}

#[test]
fn format_and_format_version_must_be_a_pair() {
    let mut only_format = v1();
    only_format
        .as_object_mut()
        .unwrap()
        .remove("format_version");
    let error = build_j(&only_format).unwrap_err();
    assert!(error.contains("$.format_version"), "{error}");

    let mut only_version = v0();
    only_version["format_version"] = json!(1);
    let error = build_j(&only_version).unwrap_err();
    assert!(error.contains("$.format"), "{error}");
}

#[test]
fn unsupported_format_version_is_explicit() {
    let mut doc = v1();
    doc["format_version"] = json!(2);
    let error = build_j(&doc).unwrap_err();
    assert!(error.contains("$.format_version"), "{error}");
    assert!(error.contains("不支持的格式版本"), "{error}");
}

#[test]
fn version_alias_must_match_format_version() {
    let mut doc = v1();
    doc["version"] = json!(2);
    let error = build_j(&doc).unwrap_err();
    assert!(error.contains("$.version"), "{error}");
    assert!(error.contains("必须等于 format_version"), "{error}");
}

#[test]
fn wrong_format_value_is_rejected() {
    let mut doc = v1();
    doc["format"] = json!("not-rpx");
    let error = build_j(&doc).unwrap_err();
    assert!(error.contains("$.format"), "{error}");
}

#[test]
fn schema_field_must_be_a_string() {
    let mut doc = v1();
    doc["schema"] = json!(1);
    let error = build_j(&doc).unwrap_err();
    assert!(error.contains("$.schema"), "{error}");
}

#[test]
fn meta_rejects_unknown_fields_and_non_string_tags() {
    let mut doc = v1();
    doc["meta"]["typo"] = json!(1);
    let error = build_j(&doc).unwrap_err();
    assert!(error.contains("$.meta.typo"), "{error}");

    let mut doc = v1();
    doc["meta"]["tags"] = json!(["ok", 1]);
    let error = build_j(&doc).unwrap_err();
    assert!(error.contains("$.meta.tags[1]"), "{error}");
}

#[test]
fn extensions_require_namespaced_keys_and_object_values() {
    let mut doc = v1();
    doc["extensions"] = json!({"acme": {}});
    let error = build_j(&doc).unwrap_err();
    assert!(error.contains("$.extensions.acme"), "{error}");

    let mut doc = v1();
    doc["extensions"] = json!({"x-acme": "not-an-object"});
    let error = build_j(&doc).unwrap_err();
    assert!(error.contains("$.extensions.x-acme"), "{error}");
}

#[test]
fn schema_advertises_v1_markers() {
    let schema = definition::schema();
    for key in [
        "format",
        "format_version",
        "schema",
        "meta",
        "extensions",
        "grid",
        "version",
    ] {
        assert!(
            schema["properties"].get(key).is_some(),
            "schema missing {key}"
        );
    }
    assert_eq!(schema["properties"]["format"]["const"], json!("rpx-report"));
    assert_eq!(schema["properties"]["format_version"]["const"], json!(1));
}

#[test]
fn validate_reports_v1_as_valid_and_keeps_legacy_v0_readable() {
    let result = definition::validate(&v1(), None).unwrap();
    assert_eq!(result["valid"], json!(true));
    let result = definition::validate(&v0(), None).unwrap();
    assert_eq!(result["valid"], json!(true));
}

#[test]
fn reportdef_build_and_definition_parse_share_the_same_reader() {
    // `definition::parse` goes through `open_report_full`; a bad v1 marker
    // must fail there too, not only in the direct `build` path.
    let mut doc = v1();
    doc["format_version"] = json!(9);
    let error = definition::parse(&doc).unwrap_err();
    assert_eq!(error.diagnostic["code"], json!("TEMPLATE_INVALID"));
    assert_eq!(error.exit, 3);
}

#[test]
fn v1_markers_do_not_change_model_shape() {
    let legacy = reportdef::build(&v0().to_string()).unwrap();
    let marked = reportdef::build(&v1().to_string()).unwrap();
    assert_eq!(legacy, marked);
}

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../../tests/fixtures/format/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn golden_corpus_valid_documents_build() {
    for name in ["minimal-v0.json", "minimal-v1.json", "legacy-slot-v1.json"] {
        build(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn golden_corpus_invalid_documents_fail_with_pointer() {
    let error = build(&fixture("invalid-format-version.json")).unwrap_err();
    assert!(error.to_string().contains("$.format_version"), "{error}");

    let error = build(&fixture("invalid-extension-key.json")).unwrap_err();
    assert!(error.to_string().contains("$.extensions.acme"), "{error}");
}
