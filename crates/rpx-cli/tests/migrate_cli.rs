//! WP2: `rpx report migrate` is JSON-only and does not require legacy-rpx.

use serde_json::{json, Value as J};
use std::{fs, path::PathBuf, process::Command};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("rpx-migrate-cli-{}", rpx_runtime::files::unique()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn invoke(args: &[&str]) -> (i32, J) {
    let out = Command::new(env!("CARGO_BIN_EXE_rpx"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        serde_json::from_slice(&out.stdout).unwrap(),
    )
}

#[test]
fn report_migrate_dry_run_and_backup() {
    let temp = Temp::new();
    let lib = temp.0.join("lib");
    let (code, value) = invoke(&["library", "init", lib.to_str().unwrap()]);
    assert_eq!(code, 0, "{value}");
    let dir = lib.join("old");
    fs::create_dir(&dir).unwrap();
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&json!({"version":1,"id":"old","name":"old","tags":[],"template":"template.json","data_dir":"data"})).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("template.json"),
        serde_json::to_vec(
            &json!({"grid":{"rows":1,"cols":1},"cells":[{"row":1,"col":1,"text":"x"}]}),
        )
        .unwrap(),
    )
    .unwrap();

    let (code, value) = invoke(&[
        "report",
        "migrate",
        "--library",
        lib.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["data"]["changed"], 1);

    let (code, value) = invoke(&[
        "report",
        "migrate",
        "--library",
        lib.to_str().unwrap(),
        "--backup",
    ]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["data"]["changed"], 1);
    let backup = PathBuf::from(value["data"]["backup"].as_str().unwrap());
    assert!(backup.join("old/template.json").is_file());
    let after: J = serde_json::from_slice(&fs::read(dir.join("template.json")).unwrap()).unwrap();
    assert_eq!(after["format_version"], 1);
    let _ = fs::remove_dir_all(backup);
}
