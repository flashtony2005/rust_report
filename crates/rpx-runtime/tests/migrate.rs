//! JSON-only library migration (WP2): v0 template → v1, with backup/rollback.

use serde_json::{json, Value as J};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("rpx-migrate-{}", rpx_runtime::files::unique()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_library(root: &std::path::Path) {
    rpx_runtime::library::init(root, false).unwrap();
    let dir = root.join("old");
    fs::create_dir(&dir).unwrap();
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&json!({
            "version":1,"id":"old","name":"old","description":"v0","tags":[],
            "template":"template.json","data_dir":"data"
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("template.json"),
        serde_json::to_vec(&json!({
            "grid":{"rows":1,"cols":1},
            "cells":[{"row":1,"col":1,"text":"hello"}]
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn dry_run_does_not_write_and_apply_creates_backup() {
    let temp = Temp::new();
    let root = &temp.0.join("lib");
    write_library(root);
    let dry = rpx_runtime::library::migrate(root, true, false).unwrap();
    assert_eq!(dry["changed"], 1);
    assert_eq!(dry["backup"], J::Null);
    let v0: J = serde_json::from_slice(&fs::read(root.join("old/template.json")).unwrap()).unwrap();
    assert!(v0.get("format").is_none(), "dry-run must not rewrite");

    let applied = rpx_runtime::library::migrate(root, false, true).unwrap();
    assert_eq!(applied["changed"], 1);
    let v1: J = serde_json::from_slice(&fs::read(root.join("old/template.json")).unwrap()).unwrap();
    assert_eq!(v1["format_version"], 1);
    let backup = PathBuf::from(applied["backup"].as_str().unwrap());
    assert!(backup.join("old/template.json").is_file());
    let backup_v0: J =
        serde_json::from_slice(&fs::read(backup.join("old/template.json")).unwrap()).unwrap();
    assert!(backup_v0.get("format").is_none());

    let again = rpx_runtime::library::migrate(root, false, false).unwrap();
    assert_eq!(again["changed"], 0);
    assert_eq!(again["unchanged"], 1);
    let _ = fs::remove_dir_all(backup);
}
