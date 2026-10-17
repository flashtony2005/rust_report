//! Real-process coverage for `rpx format ...`.

use serde_json::{json, Value as J};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("rpx-format-cli-{}", rpx_runtime::files::unique()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn invoke(args: &[&str]) -> (i32, J, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_rpx"))
        .args(args)
        .output()
        .unwrap();
    let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout is not JSON: {:?}; stderr={:?}",
            output.stdout, output.stderr
        )
    });
    (
        output.status.code().unwrap_or(-1),
        value,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn format_fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/format")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn verify_and_byte_roundtrip_a_product_rpx() {
    let (code, value, stderr) = invoke(&[
        "format",
        "verify",
        &fixture("engine_merge.rpx"),
        "--mode",
        "preserve",
    ]);
    assert_eq!(code, 0, "{value} {stderr}");
    assert_eq!(value["data"]["model_equal"], json!(true));
    assert_eq!(value["data"]["bytes_equal"], json!(true));
    assert_eq!(value["data"]["legacy"], json!(true));
}

#[test]
fn to_json_then_to_rpx_is_byte_identical() {
    let temp = Temp::new();
    let json_path = temp.path("engine_merge.json");
    let rpx_path = temp.path("engine_merge.rpx");
    let (code, value, _) = invoke(&[
        "format",
        "to-json",
        &fixture("engine_merge.rpx"),
        "--out",
        &json_path,
        "--mode",
        "preserve",
    ]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["data"]["legacy"], json!(true));
    assert!(Path::new(&json_path).is_file());

    let (code, value, _) = invoke(&[
        "format", "to-rpx", &json_path, "--out", &rpx_path, "--mode", "preserve",
    ]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(
        fs::read(&rpx_path).unwrap(),
        fs::read(fixture("engine_merge.rpx")).unwrap()
    );
}

#[test]
fn strict_mode_rejects_unmodeled_fields() {
    let temp = Temp::new();
    let out = temp.path("strict.json");
    let (code, value, _) = invoke(&[
        "format",
        "to-json",
        &fixture("engine_merge.rpx"),
        "--out",
        &out,
        "--mode",
        "strict",
    ]);
    assert_eq!(code, 3, "{value}");
    assert_eq!(value["error"]["code"], json!("FORMAT_UNSUPPORTED_FIELD"));
    assert!(
        !Path::new(&out).exists(),
        "strict must not write a partial file"
    );
}

#[test]
fn check_accepts_v1_fixture_and_rejects_bad_extension() {
    let (code, value, _) = invoke(&["format", "check", &format_fixture("minimal-v1.json")]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["data"]["valid"], json!(true));

    let (code, value, _) = invoke(&[
        "format",
        "check",
        &format_fixture("invalid-extension-key.json"),
    ]);
    assert_eq!(code, 3, "{value}");
    assert_eq!(value["error"]["code"], json!("TEMPLATE_INVALID"));
}

#[test]
fn edited_preserve_document_is_read_only() {
    let temp = Temp::new();
    let json_path = temp.path("edited.json");
    let rpx_path = temp.path("edited.rpx");
    let (code, value, _) = invoke(&[
        "format",
        "to-json",
        &fixture("engine_merge.rpx"),
        "--out",
        &json_path,
        "--mode",
        "preserve",
    ]);
    assert_eq!(code, 0, "{value}");
    let mut doc: J = serde_json::from_slice(&fs::read(&json_path).unwrap()).unwrap();
    doc["grid"]["rows"] = json!(1);
    fs::write(&json_path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    let (code, value, _) = invoke(&[
        "format", "to-rpx", &json_path, "--out", &rpx_path, "--mode", "preserve",
    ]);
    assert_eq!(code, 3, "{value}");
    assert_eq!(value["error"]["code"], json!("LEGACY_READ_ONLY"));
}

#[test]
fn normalize_adds_v1_markers() {
    let temp = Temp::new();
    let out = temp.path("normalized.json");
    let (code, value, _) = invoke(&[
        "format",
        "normalize",
        &format_fixture("minimal-v0.json"),
        "--out",
        &out,
    ]);
    assert_eq!(code, 0, "{value}");
    let doc: J = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
    assert_eq!(doc["format"], "rpx-report");
    assert_eq!(doc["format_version"], 1);
    assert_eq!(doc["schema"], "rpx-report/v1");
    let (code, value, _) = invoke(&["format", "check", &out]);
    assert_eq!(code, 0, "{value}");
}
