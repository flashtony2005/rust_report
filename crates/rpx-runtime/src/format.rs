//! JSON v1 格式边界：`.rpx` 迁移、JSON → 模型/字节、往返验证。
//!
//! 见 `docs/format/rpx-report-json-v1.md` 与
//! `docs/format/rpx-core-json-converter.md`。Phase 1 的 Preserve 模式把原始
//! `.rpx` 字节放进 `extensions["rpx.legacy"]`（迁移快照）；对已编辑的可建模
//! 字段会显式拒绝回写，避免“看起来无损、实际丢字段”。

#[cfg(feature = "legacy-rpx")]
mod legacy;
#[cfg(feature = "legacy-rpx")]
mod modeled;
#[cfg(feature = "legacy-rpx")]
mod overlay;

#[cfg(feature = "legacy-rpx")]
use crate::error::Error;
use crate::{
    definition,
    error::Result,
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
    /// 允许含快照的结构性编辑（尾部增删/坐标不变的重叠区）；默认拒绝。
    pub allow_structural: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::Strict,
            allow_structural: false,
        }
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
#[cfg(feature = "legacy-rpx")]
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

/// 把 v0/v1 定义规范化为 v1（补 `format`/`format_version`/`schema`）。
pub fn normalize(doc: &J) -> Result<J> {
    definition::parse(doc)?;
    Ok(with_markers(doc.clone()))
}

/// v1/v0 JSON → [`ReportDefine`]；忽略 `extensions`。
pub fn json_to_model(doc: &J) -> Result<ReportDefine> {
    #[cfg(feature = "legacy-rpx")]
    reject_corrupt_legacy(doc)?;
    #[allow(unused_mut)]
    let (mut rd, _) = definition::parse(doc)?;
    #[cfg(feature = "legacy-rpx")]
    overlay::apply(doc, &mut rd);
    Ok(rd)
}

#[cfg(feature = "legacy-rpx")]
fn has_legacy(doc: &J) -> bool {
    doc.get("extensions")
        .and_then(|v| v.get(legacy::KEY))
        .is_some()
}

#[cfg(feature = "legacy-rpx")]
fn reject_corrupt_legacy(doc: &J) -> Result<()> {
    if has_legacy(doc) && legacy::read(doc).is_none() {
        return Err(Error::new(
            3,
            "LEGACY_INVALID",
            "format",
            "extensions.rpx.legacy 存在但快照无效（base64/sha256/format 不匹配）",
        )
        .at("/extensions/rpx.legacy"));
    }
    Ok(())
}

/// v1/v0 JSON → `.rpx` 字节。
#[cfg(feature = "legacy-rpx")]
///
/// Preserve 模式且存在未被编辑的迁移快照时直接返回原字节；一旦可建模字段被
/// 编辑，用字段级 overlay 把未建模字段合并回模型再写出。语义保留，不保证
/// 字节相等；原始字节路径只用于“未编辑”的文件。
pub fn json_to_rpx(doc: &J, opts: Options) -> Result<Vec<u8>> {
    if opts.mode != Mode::DropLegacy {
        reject_corrupt_legacy(doc)?;
        if let Some(snapshot) = legacy::read(doc) {
            if legacy::modeled_sha256(doc) == snapshot.modeled_sha256 {
                return Ok(snapshot.bytes);
            }
            // 结构性编辑（增删行列）会让未建模槽失去宿主；Phase 2 显式拒绝，
            // 等字段级 overlay 支持结构变更后再放开。
            let (parsed, _) = definition::parse(doc)?;
            let original = rpx_core::open_bytes(&snapshot.bytes)
                .map_err(|e| Error::new(3, "RPX_INVALID", "format", e))?;
            if (parsed.rows != original.rows || parsed.cols != original.cols)
                && !opts.allow_structural
            {
                return Err(Error::new(
                    3,
                    "LEGACY_STRUCTURAL_EDIT",
                    "format",
                    "含 RPX 迁移快照时暂不支持增删行列；请加 --allow-structural（仅保证重叠区）或先移除 extensions.rpx.legacy",
                )
                .at("/extensions/rpx.legacy"));
            }
            // 已编辑：用字段级 overlay 把未建模字段合并回模型，再走普通写出。
            // 语义保留；不保证字节相等（原始字节路径只用于“未编辑”）。
        }
    }
    let rd = json_to_model(doc)?;
    rpx_core::to_bytes(&rd).map_err(|e| Error::new(6, "RPX_COMPILE", "format", e))
}

/// 读 → JSON → 模型/字节 → 再读；用于迁移验收。
#[cfg(feature = "legacy-rpx")]
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

#[cfg(feature = "legacy-rpx")]
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

#[cfg(all(test, feature = "legacy-rpx"))]
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
                allow_structural: false,
            },
        )
        .unwrap();
        assert!(converted.legacy);
        assert!(converted.document["extensions"]["rpx.legacy"].is_object());
        let rebuilt = json_to_rpx(
            &converted.document,
            Options {
                mode: Mode::PreserveLegacy,
                allow_structural: false,
            },
        )
        .unwrap();
        assert_eq!(rebuilt, bytes);
    }

    #[test]
    fn editing_snapshot_json_uses_overlay() {
        let doc = json!({
            "format": "rpx-report",
            "format_version": 1,
            "grid": {"rows": 1, "cols": 1},
            "cells": [{"row": 1, "col": 1, "text": "hello"}],
        });
        let bytes = json_to_rpx(&doc, Options::default()).unwrap();
        let mut converted = rpx_to_json(
            &bytes,
            Options {
                mode: Mode::PreserveLegacy,
                allow_structural: false,
            },
        )
        .unwrap();
        converted.document["cells"][0]["font_size"] = json!(13);
        let rebuilt = json_to_rpx(
            &converted.document,
            Options {
                mode: Mode::PreserveLegacy,
                allow_structural: false,
            },
        )
        .unwrap();
        let rd = rpx_core::open_bytes(&rebuilt).unwrap();
        assert_eq!(rd.cell_at(1, 1).unwrap().font_size, 13);
    }
}
