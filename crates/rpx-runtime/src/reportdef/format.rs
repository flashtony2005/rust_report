//! v1 格式标记、meta 与 extensions 的语义校验。
//!
//! `strict` 只按 schema 检查字段名；本模块补上 `format`/`format_version` 的配对、
//! `version` 别名一致性，以及 `extensions` 的命名空间规则。v0 文件（没有
//! `format`）继续原样接受。

use serde_json::{Map, Value as J};

use super::json::{arr, as_str, as_u64, err, obj};
use super::parse::{FORMAT, FORMAT_VERSION};
use super::DefError;

/// v1 格式标记与扩展位的语义校验。
///
/// `strict` 只按 schema 检查字段名；这里补上 `format`/`format_version` 的配对、
/// `version` 别名一致性，以及 `extensions` 的命名空间规则。v0 文件（没有
/// `format`）继续原样接受。
pub(super) fn validate(root: &Map<String, J>) -> Result<(), DefError> {
    let format = root.get("format");
    let format_version = root.get("format_version");
    if format.is_some() != format_version.is_some() {
        return Err(err(
            if format.is_some() {
                "$.format_version"
            } else {
                "$.format"
            },
            "format 与 format_version 必须同时声明",
        ));
    }
    if let (Some(format), Some(format_version)) = (format, format_version) {
        let format = as_str(format, "$.format")?;
        if format != FORMAT {
            return Err(err(
                "$.format",
                format!("应为 \"{FORMAT}\"（实得 {format:?}）"),
            ));
        }
        let n = as_u64(format_version, "$.format_version")?;
        if n != FORMAT_VERSION {
            return Err(err(
                "$.format_version",
                format!("不支持的格式版本 {n}（本实现支持 {FORMAT_VERSION}）"),
            ));
        }
        if let Some(v) = root.get("version") {
            if as_u64(v, "$.version")? != n {
                return Err(err("$.version", "必须等于 format_version"));
            }
        }
    }
    if let Some(schema) = root.get("schema") {
        as_str(schema, "$.schema")?;
    }
    if let Some(meta) = root.get("meta") {
        validate_meta(meta)?;
    }
    if let Some(extensions) = root.get("extensions") {
        validate_extensions(extensions)?;
    }
    Ok(())
}

fn validate_meta(v: &J) -> Result<(), DefError> {
    let o = obj(v, "$.meta")?;
    for key in o.keys() {
        if !matches!(key.as_str(), "title" | "description" | "tags") {
            return Err(err(&format!("$.meta.{key}"), "未知字段"));
        }
    }
    for key in ["title", "description"] {
        if let Some(value) = o.get(key) {
            as_str(value, &format!("$.meta.{key}"))?;
        }
    }
    if let Some(tags) = o.get("tags") {
        for (i, tag) in arr(tags, "$.meta.tags")?.iter().enumerate() {
            as_str(tag, &format!("$.meta.tags[{i}]"))?;
        }
    }
    Ok(())
}

fn validate_extensions(v: &J) -> Result<(), DefError> {
    let o = obj(v, "$.extensions")?;
    for (key, value) in o {
        if !(key.starts_with("rpx.") || key.starts_with("x-")) {
            return Err(err(
                &format!("$.extensions.{key}"),
                "扩展键必须形如 rpx.* 或 x-<vendor>",
            ));
        }
        if !value.is_object() {
            return Err(err(&format!("$.extensions.{key}"), "扩展值必须是对象"));
        }
    }
    Ok(())
}
