//! JSON 取值助手：**把「类型不对」变成带路径的 `DefError`**。
//!
//! 与 `parse.rs` 分开的理由：`parse.rs` 管**语义**（哪些字段、什么取值合法），
//! 本文件管**形态**（这地方到底是不是对象/数组/字符串/非负整数）。
//! 两类错误混在一个文件里最容易互相抄错（本仓在 `expr/dsstate.rs` 踩过同款）。
//!
//! 每条错误都带 JSON 路径（如 `$.cells[3].merge.rows`）——错误消息是契约的一部分
//! （见 `docs/report-def-json.md` §6.2），前端据此定位到具体字段。

use serde_json::{Map, Value as J};

use super::DefError;

/// 造一条带路径的 `Schema` 错。
pub(super) fn err(path: &str, detail: impl Into<String>) -> DefError {
    DefError::Schema {
        path: path.to_string(),
        detail: detail.into(),
    }
}

/// JSON 值的中文类型名（错误消息用）。
pub(super) fn kind_of(v: &J) -> &'static str {
    match v {
        J::Null => "null",
        J::Bool(_) => "布尔",
        J::Number(_) => "数字",
        J::String(_) => "字符串",
        J::Array(_) => "数组",
        J::Object(_) => "对象",
    }
}

/// 取对象。
pub(super) fn obj<'a>(v: &'a J, path: &str) -> Result<&'a Map<String, J>, DefError> {
    v.as_object()
        .ok_or_else(|| err(path, format!("应为对象，实得 {}", kind_of(v))))
}

/// 取数组。
pub(super) fn arr<'a>(v: &'a J, path: &str) -> Result<&'a Vec<J>, DefError> {
    v.as_array()
        .ok_or_else(|| err(path, format!("应为数组，实得 {}", kind_of(v))))
}

/// 取必填字段。
pub(super) fn field<'a>(o: &'a Map<String, J>, key: &str, path: &str) -> Result<&'a J, DefError> {
    o.get(key)
        .ok_or_else(|| err(&format!("{path}.{key}"), "缺少必填字段"))
}

/// 取非负整数。
pub(super) fn as_u64(v: &J, path: &str) -> Result<u64, DefError> {
    v.as_u64()
        .ok_or_else(|| err(path, format!("应为非负整数，实得 {}", kind_of(v))))
}

/// 取非负整数（`usize` 形态）。
pub(super) fn as_usize(v: &J, path: &str) -> Result<usize, DefError> {
    Ok(as_u64(v, path)? as usize)
}

/// 取浮点数（`f64` → `f32`）。
///
/// 只认 JSON number：**不接字符串**。行高/列宽接受「字符串 = 表达式」是因为
/// 那是属性槽（§4.4），而缩进/旋转/纸张尺寸是**裸数值槽**，没有表达式形态
/// （`ReportDefine` 里它们是 `f32` 直写，见附件 B.3.2 / D）⇒ 给字符串一律报错，
/// 不猜它是不是表达式。
pub(super) fn as_f32(v: &J, path: &str) -> Result<f32, DefError> {
    v.as_f64()
        .map(|n| n as f32)
        .ok_or_else(|| err(path, format!("应为数字，实得 {}", kind_of(v))))
}

/// 取可选浮点数（缺省 ⇒ `None`）。
pub(super) fn opt_f32(o: &Map<String, J>, key: &str, path: &str) -> Result<Option<f32>, DefError> {
    match o.get(key) {
        None => Ok(None),
        Some(v) => Ok(Some(as_f32(v, &format!("{path}.{key}"))?)),
    }
}

/// 取可选非负整数（缺省 ⇒ `None`）。
pub(super) fn opt_u64(o: &Map<String, J>, key: &str, path: &str) -> Result<Option<u64>, DefError> {
    match o.get(key) {
        None => Ok(None),
        Some(v) => Ok(Some(as_u64(v, &format!("{path}.{key}"))?)),
    }
}

/// 取可选布尔（缺省 ⇒ `None`）。
pub(super) fn opt_bool(
    o: &Map<String, J>,
    key: &str,
    path: &str,
) -> Result<Option<bool>, DefError> {
    match o.get(key) {
        None => Ok(None),
        Some(v) => v.as_bool().map(Some).ok_or_else(|| {
            err(
                &format!("{path}.{key}"),
                format!("应为布尔，实得 {}", kind_of(v)),
            )
        }),
    }
}

/// 取字符串。
pub(super) fn as_str<'a>(v: &'a J, path: &str) -> Result<&'a str, DefError> {
    v.as_str()
        .ok_or_else(|| err(path, format!("应为字符串，实得 {}", kind_of(v))))
}

/// 取可选字符串字段（缺省 ⇒ `None`）。
pub(super) fn opt_str<'a>(
    o: &'a Map<String, J>,
    key: &str,
    path: &str,
) -> Result<Option<&'a str>, DefError> {
    match o.get(key) {
        None => Ok(None),
        Some(v) => Ok(Some(as_str(v, &format!("{path}.{key}"))?)),
    }
}
