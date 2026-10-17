//! RPX 迁移快照（Phase 1 的 legacy 舱）。
//!
//! 设计取舍：v1 目前没有为全部产品字段建 JSON 表达；为了让 40 个产品 `.rpx`
//! 都能先“安全迁移”，Preserve 模式把**原始 `.rpx` 字节**放进
//! `extensions["rpx.legacy"]`，并记录可建模部分的哈希。只要 JSON 的可建模
//! 部分未被编辑，`json_to_rpx` 可以原样还回原字节；一旦编辑，明确报
//! `LEGACY_READ_ONLY`，要求先移除 legacy 舱再导出（或等待 Phase 2 的逐字段
//! 覆盖实现）。
//!
//! 这不是长期编辑格式；它是迁移/验收的安全绳。

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Map, Value as J};
use sha2::{Digest, Sha256};

pub const KEY: &str = "rpx.legacy";
pub const FORMAT: &str = "rpx-rpx-v1";

/// 可建模字段（影响 `ReportDefine` 的 JSON 键）。
const MODEL_KEYS: &[&str] = &[
    "version",
    "grid",
    "cells",
    "row_props",
    "col_props",
    "datasets",
    "params",
    "print_setup",
    "printing",
    "group_breaks",
    "background_image",
    "macros",
];

pub struct Snapshot {
    pub bytes: Vec<u8>,
    pub modeled_sha256: String,
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// 仅对“会影响模型”的键做规范化哈希；`meta` / `extensions` 不参与。
pub fn modeled_sha256(doc: &J) -> String {
    let Some(object) = doc.as_object() else {
        return sha256(b"");
    };
    let mut selected = Map::new();
    for key in MODEL_KEYS {
        if let Some(value) = object.get(*key) {
            selected.insert((*key).into(), canonical(value));
        }
    }
    sha256(
        serde_json::to_string(&canonical(&J::Object(selected)))
            .unwrap_or_default()
            .as_bytes(),
    )
}

/// 给文档挂上迁移快照；已有扩展保留。
pub fn attach(doc: &mut J, bytes: &[u8]) {
    let snapshot = json!({
        "format": FORMAT,
        "encoding": "base64",
        "sha256": sha256(bytes),
        "modeled_sha256": modeled_sha256(doc),
        "data": STANDARD.encode(bytes),
    });
    let object = doc.as_object_mut().expect("document is object");
    let extensions = object
        .entry("extensions")
        .or_insert_with(|| J::Object(Map::new()));
    if !extensions.is_object() {
        *extensions = J::Object(Map::new());
    }
    extensions
        .as_object_mut()
        .expect("extensions object")
        .insert(KEY.into(), snapshot);
}

/// 读取快照；不存在/形态不对返回 `None`，让调用方走普通 JSON 路径。
pub fn read(doc: &J) -> Option<Snapshot> {
    let leg = doc.get("extensions")?.get(KEY)?;
    if leg.get("format")?.as_str()? != FORMAT {
        return None;
    }
    let encoded = leg.get("data")?.as_str()?;
    let bytes = STANDARD.decode(encoded).ok()?;
    let expected = leg.get("sha256")?.as_str()?;
    if sha256(&bytes) != expected {
        return None;
    }
    let modeled_sha256 = leg.get("modeled_sha256")?.as_str()?.to_string();
    Some(Snapshot {
        bytes,
        modeled_sha256,
    })
}

/// 移除 legacy 舱，返回是否移除。
#[allow(dead_code)]
pub fn remove(doc: &mut J) -> bool {
    let Some(extensions) = doc.get_mut("extensions").and_then(J::as_object_mut) else {
        return false;
    };
    extensions.remove(KEY).is_some()
}

/// 递归 key 排序的稳定 JSON（仅用于哈希/比较，不用于落盘）。
fn canonical(v: &J) -> J {
    match v {
        J::Object(o) => {
            let mut keys: Vec<_> = o.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for key in keys {
                out.insert(key.clone(), canonical(&o[key]));
            }
            J::Object(out)
        }
        J::Array(a) => J::Array(a.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modeled_hash_ignores_meta_and_extensions() {
        let a = json!({"grid":{"rows":1,"cols":1},"meta":{"title":"a"}});
        let b = json!({"grid":{"rows":1,"cols":1},"meta":{"title":"b"},
            "extensions":{"x-a":{"n":1}}});
        assert_eq!(modeled_sha256(&a), modeled_sha256(&b));
    }

    #[test]
    fn modeled_hash_tracks_modeled_fields() {
        let a = json!({"grid":{"rows":1,"cols":1}});
        let b = json!({"grid":{"rows":1,"cols":2}});
        assert_ne!(modeled_sha256(&a), modeled_sha256(&b));
    }

    #[test]
    fn attach_and_read_roundtrip() {
        let mut doc = json!({"grid":{"rows":1,"cols":1}});
        attach(&mut doc, b"RQQR-raw");
        let snapshot = read(&doc).expect("snapshot");
        assert_eq!(snapshot.bytes, b"RQQR-raw");
        assert!(!snapshot.modeled_sha256.is_empty());
        assert!(remove(&mut doc));
        assert!(read(&doc).is_none());
    }

    #[test]
    fn corrupted_snapshot_is_not_silently_used() {
        let mut doc = json!({"grid":{"rows":1,"cols":1}});
        attach(&mut doc, b"RQQR-raw");
        doc["extensions"][KEY]["data"] = json!("AAAA");
        assert!(read(&doc).is_none());
    }
}
