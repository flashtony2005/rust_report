//! JSON v1 格式边界：`.rpx` 迁移、JSON → 模型/字节、往返验证。
//!
//! 见 `docs/format/rpx-report-json-v1.md` 与
//! `docs/format/rpx-core-json-converter.md`。Phase 1 的 Preserve 模式把原始
//! `.rpx` 字节放进 `extensions["rpx.legacy"]`（迁移快照）；对已编辑的可建模
//! 字段会显式拒绝回写，避免“看起来无损、实际丢字段”。

mod legacy;
mod modeled;

use crate::{
    definition,
    error::{Error, Result},
    reportdef::{FORMAT, FORMAT_VERSION},
};
use rpx_core::ReportDefine;
use serde_json::Value as J;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 只接受 v1 可表达的字段；遇到未建模字段失败。
    Strict,
    /// 产出 v1 + `extensions["rpx.legacy"]` 原始快照；迁移默认。
    PreserveLegacy,
    /// 丢弃未建模字段；只用于渲染。
    DropLegacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub mode: Mode,
}

impl Default for Options {
    fn default() -> Self {
        Self { mode: Mode::Strict }
    }
}

#[derive(Debug, Clone)]
pub struct Converted {
    pub document: J,
    pub warnings: Vec<String>,
    pub legacy: bool,
}

#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub model_equal: bool,
    pub bytes_equal: bool,
    pub legacy: bool,
    pub source_sha256: String,
    pub json_sha256: String,
}

/// `.rpx` → v1 JSON。`PreserveLegacy` 会挂原始字节快照。
pub fn rpx_to_json(bytes: &[u8], opts: Options) -> Result<Converted> {
    let rd = rpx_core::open_bytes(bytes).map_err(|e| Error::new(3, "RPX_INVALID", "format", e))?;
    if opts.mode == Mode::Strict {
        let (doc, warnings) = modeled::document(&rd);
        // 先验证可建模结果真的能被 reader 接受；Strict 不能“生成了 JSON 但读不回”。
        definition::parse(&doc)?;
        if let Some(warning) = warnings.first() {
            return Err(unsupported(warning));
        }
        return Ok(Converted {
            document: with_markers(doc),
            warnings,
            legacy: false,
        });
    }

    let (doc, warnings) = modeled::document(&rd);
    let mut document = with_markers(doc);
    if opts.mode == Mode::PreserveLegacy {
        legacy::attach(&mut document, bytes);
    }
    // 最终产物必须能被 reader 读回；否则 Preserve 只是“看起来成功”。
    definition::parse(&document)?;
    Ok(Converted {
        document,
        warnings,
        legacy: opts.mode == Mode::PreserveLegacy,
    })
}

/// v1/v0 JSON → [`ReportDefine`]；忽略 `extensions`。
pub fn json_to_model(doc: &J) -> Result<ReportDefine> {
    definition::parse(doc).map(|(rd, _)| rd)
}

/// v1/v0 JSON → `.rpx` 字节。
///
/// Preserve 模式且存在未被编辑的迁移快照时直接返回原字节；一旦可建模字段被
/// 编辑，返回 `LEGACY_READ_ONLY`，要求先移除快照再导出。这样不会出现
/// “编辑了 JSON，但写回时丢了未建模字段”的静默损失。
pub fn json_to_rpx(doc: &J, opts: Options) -> Result<Vec<u8>> {
    if opts.mode != Mode::DropLegacy {
        if let Some(snapshot) = legacy::read(doc) {
            if legacy::modeled_sha256(doc) == snapshot.modeled_sha256 {
                return Ok(snapshot.bytes);
            }
            return Err(Error::new(
                3,
                "LEGACY_READ_ONLY",
                "format",
                "该 JSON 含 RPX 迁移快照，且可建模字段已被编辑；请先移除 extensions.rpx.legacy 再导出，或重新从 .rpx 迁移",
            )
            .at("/extensions/rpx.legacy"));
        }
    }
    let rd = json_to_model(doc)?;
    rpx_core::to_bytes(&rd).map_err(|e| Error::new(6, "RPX_COMPILE", "format", e))
}

/// 读 → JSON → 模型/字节 → 再读；用于迁移验收。
pub fn verify(bytes: &[u8], opts: Options) -> Result<VerifyReport> {
    let original =
        rpx_core::open_bytes(bytes).map_err(|e| Error::new(3, "RPX_INVALID", "format", e))?;
    let converted = rpx_to_json(bytes, opts)?;
    let rebuilt = json_to_rpx(&converted.document, opts)?;
    let actual =
        rpx_core::open_bytes(&rebuilt).map_err(|e| Error::new(3, "RPX_INVALID", "format", e))?;
    Ok(VerifyReport {
        model_equal: original == actual,
        bytes_equal: rebuilt == bytes,
        legacy: converted.legacy,
        source_sha256: legacy::sha256(bytes),
        json_sha256: legacy::sha256(&serde_json::to_vec(&converted.document)?),
    })
}

fn with_markers(mut doc: J) -> J {
    let object = doc.as_object_mut().expect("modeled document is object");
    object.insert("format".into(), serde_json::json!(FORMAT));
    object.insert("format_version".into(), serde_json::json!(FORMAT_VERSION));
    object.insert("schema".into(), serde_json::json!("rpx-report/v1"));
    doc
}

fn unsupported(path: &str) -> Error {
    let pointer = format!("/{}", path.replace('.', "/"));
    Error::new(
        3,
        "FORMAT_UNSUPPORTED_FIELD",
        "format",
        format!("JSON v1 尚不能表达 {path}；PreserveLegacy 可用原始快照迁移"),
    )
    .at(pointer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn v1_minimal_json_roundtrips_to_rpx() {
        let doc = json!({
            "format": "rpx-report",
            "format_version": 1,
            "grid": {"rows": 1, "cols": 1},
            "cells": [{"row": 1, "col": 1, "text": "hello"}],
        });
        let bytes = json_to_rpx(&doc, Options::default()).unwrap();
        let rd = rpx_core::open_bytes(&bytes).unwrap();
        assert_eq!(rd.rows, 2);
        assert_eq!(rd.cols, 2);
    }

    #[test]
    fn preserve_snapshot_returns_original_bytes_when_unescaped() {
        let doc = json!({
            "format": "rpx-report",
            "format_version": 1,
            "grid": {"rows": 1, "cols": 1},
        });
        let bytes = json_to_rpx(&doc, Options::default()).unwrap();
        let converted = rpx_to_json(
            &bytes,
            Options {
                mode: Mode::PreserveLegacy,
            },
        )
        .unwrap();
        assert!(converted.legacy);
        assert!(converted.document["extensions"]["rpx.legacy"].is_object());
        let rebuilt = json_to_rpx(
            &converted.document,
            Options {
                mode: Mode::PreserveLegacy,
            },
        )
        .unwrap();
        assert_eq!(rebuilt, bytes);
    }

    #[test]
    fn editing_snapshot_json_is_rejected() {
        let doc = json!({
            "format": "rpx-report",
            "format_version": 1,
            "grid": {"rows": 1, "cols": 1},
        });
        let bytes = json_to_rpx(&doc, Options::default()).unwrap();
        let mut converted = rpx_to_json(
            &bytes,
            Options {
                mode: Mode::PreserveLegacy,
            },
        )
        .unwrap();
        converted.document["grid"]["rows"] = json!(2);
        let error = json_to_rpx(
            &converted.document,
            Options {
                mode: Mode::PreserveLegacy,
            },
        )
        .unwrap_err();
        assert_eq!(error.diagnostic["code"], json!("LEGACY_READ_ONLY"));
    }
}
