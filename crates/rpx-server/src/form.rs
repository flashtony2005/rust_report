//! multipart 表单 → [`RenderRequest`]（`POST /render` 请求体，M3-05）。
//!
//! # 字段约定（`docs/spec-m3.md` §3.2 主链路 ① 的 HTTP 版）
//!
//! | 字段      | 次数 | 内容                                                                 |
//! | --------- | ---- | -------------------------------------------------------------------- |
//! | `report`  | 1    | `.rpx` 文件字节（魔数由 `rpx_core::open_bytes` 校验）                  |
//! | `dataset` | 0..n | 数据集 CSV；multipart `filename` 必须与模板 `FileDataSetConfig.file_name` 一致（如 `engine_sum.csv`） |
//! | `params`  | 0..1 | 参数 JSON 对象（`{"p": [1,2,3]}`）；缺省 = 空参数表                    |
//!
//! 未知字段**显式拒绝**（不静默忽略）——「诚实失败优于静默错」。
//!
//! # 参数值推断口径
//!
//! 与引擎测试夹具 loader（`crates/rpx-engine/tests/common/loader.rs` 的
//! `parse_param_value`）互逆：整数 → [`Value::Int`]、小数/指数 → [`Value::Double`]、
//! 数组 → [`Value::List`]、字符串 → [`Value::Text`]、布尔/null 直通。
//! 差异一处：loader 按 token 原文分家、超大整数走 `as_i32`（饱和），这里按
//! serde_json 数值模型 —— 入 i32 → `Int`，否则 `Long`（见 [`json_to_value`]）。

use std::collections::BTreeMap;

use axum::extract::Multipart;
use rpx_engine::params::ParamTable;
use rpx_engine::value::Value;
use serde_json::Value as J;

/// `POST /render` 的解析产物（主链路 [`crate::http::render_request`] 的输入）。
pub struct RenderRequest {
    /// `.rpx` 字节（魔数由 `rpx_core::open_bytes` 校验）。
    pub report: Vec<u8>,
    /// 报表名（multipart `filename`；进快照 `name` 槽，仅标识用）。
    pub report_name: String,
    /// 数据集 CSV：键 = `FileDataSetConfig.file_name`，值 = 文件字节。
    pub datasets: BTreeMap<String, Vec<u8>>,
    /// 参数表（`params` 字段；缺省 = 空表，等价无参入口 `calculate`）。
    pub params: ParamTable,
}

/// 表单错误 —— 一律是客户端输入问题（⇒ HTTP 400，见 [`crate::http`]）。
#[derive(Debug)]
pub struct FormError(pub String);

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// 解析 multipart 表单（字段约定见模块文档）。
pub async fn parse(mp: &mut Multipart) -> Result<RenderRequest, FormError> {
    let mut report: Option<(Vec<u8>, Option<String>)> = None;
    let mut datasets: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut params_text: Option<String> = None;
    while let Some(field) = mp
        .next_field()
        .await
        .map_err(|e| FormError(format!("multipart 读取失败：{e}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        let filename = field.file_name().map(str::to_string);
        match name.as_str() {
            "report" => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| FormError(format!("report 字段读取失败：{e}")))?;
                report = Some((bytes.to_vec(), filename));
            }
            "dataset" => {
                let Some(fname) = filename else {
                    return Err(FormError(
                        "dataset 字段缺少 filename（无法与模板 file_name 对齐）".to_string(),
                    ));
                };
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| FormError(format!("dataset {fname} 读取失败：{e}")))?;
                if datasets.insert(fname.clone(), bytes.to_vec()).is_some() {
                    return Err(FormError(format!("重复的数据集文件：{fname}")));
                }
            }
            "params" => {
                params_text = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| FormError(format!("params 字段读取失败：{e}")))?,
                );
            }
            other => {
                return Err(FormError(format!(
                    "未知字段：{other}（只接受 report / dataset / params）"
                )));
            }
        }
    }
    let Some((report, report_name)) = report else {
        return Err(FormError("缺少 report 字段（.rpx 文件）".to_string()));
    };
    let params = match params_text {
        Some(text) => parse_params(&text)?,
        None => ParamTable::EMPTY,
    };
    Ok(RenderRequest {
        report,
        report_name: derive_report_name(report_name),
        datasets,
        params,
    })
}

/// 报表名 = 文件名去扩展名（与 dev bin 的 `Path::file_stem` 同口径 ——
/// 快照 `name`/`id` 槽两条路径必须一致，否则 HTTP 与库输出无可比性）。
fn derive_report_name(filename: Option<String>) -> String {
    filename
        .as_deref()
        .and_then(|f| std::path::Path::new(f).file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("report")
        .to_string()
}

/// `params` JSON 对象 → [`ParamTable`]。
fn parse_params(text: &str) -> Result<ParamTable, FormError> {
    let j: J =
        serde_json::from_str(text).map_err(|e| FormError(format!("params 不是合法 JSON：{e}")))?;
    let J::Object(map) = j else {
        return Err(FormError(
            "params 必须是 JSON 对象（如 {\"p\": [1,2,3]}）".to_string(),
        ));
    };
    map.into_iter()
        .map(|(k, v)| Ok((k, json_to_value(&v)?)))
        .collect()
}

/// JSON 值 → [`Value`]（口径见模块文档；对象形态显式拒绝，与夹具 loader 的
/// panic 同义 —— 产品 `setParamValue` 注入面从未见过对象）。
fn json_to_value(j: &J) -> Result<Value, FormError> {
    match j {
        J::Null => Ok(Value::Null),
        J::Bool(b) => Ok(Value::Bool(*b)),
        J::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i32::try_from(i).map(Value::Int).unwrap_or(Value::Long(i)))
            } else if let Some(u) = n.as_u64() {
                Ok(i32::try_from(u)
                    .map(Value::Int)
                    .unwrap_or(Value::Long(u as i64)))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Double(f))
            } else {
                unreachable!("serde_json 默认特性已禁非有限数")
            }
        }
        J::String(s) => Ok(Value::Text(s.clone())),
        J::Array(a) => a
            .iter()
            .map(json_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        J::Object(_) => Err(FormError(
            "参数值出现对象形态 —— 产品注入面（setParamValue）未见过，先补探针".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_numbers_split_by_shape() {
        // 与夹具 loader 的 token 口径对齐：1 → Int、1.5/1e2 → Double。
        assert_eq!(json_to_value(&J::from(1)).unwrap(), Value::Int(1));
        assert_eq!(json_to_value(&J::from(-5)).unwrap(), Value::Int(-5));
        assert_eq!(json_to_value(&J::from(1.5)).unwrap(), Value::Double(1.5));
        assert_eq!(
            json_to_value(&serde_json::from_str::<J>("1e2").unwrap()).unwrap(),
            Value::Double(100.0)
        );
        // 超大整数：入不了 i32 的一律 Long（不饱和成 Int）。
        assert_eq!(
            json_to_value(&serde_json::json!(5_000_000_000i64)).unwrap(),
            Value::Long(5_000_000_000)
        );
    }

    #[test]
    fn json_lists_and_scalars_map_directly() {
        assert_eq!(json_to_value(&J::Null).unwrap(), Value::Null);
        assert_eq!(json_to_value(&J::from(true)).unwrap(), Value::Bool(true));
        assert_eq!(
            json_to_value(&J::from("x")).unwrap(),
            Value::Text("x".into())
        );
        assert_eq!(
            json_to_value(&serde_json::json!([1, "a", null])).unwrap(),
            Value::List(vec![Value::Int(1), Value::Text("a".into()), Value::Null])
        );
    }

    #[test]
    fn object_param_value_is_refused() {
        let err = json_to_value(&serde_json::json!({"a": 1})).unwrap_err();
        assert!(err.0.contains("对象形态"), "实得：{err:?}");
    }

    #[test]
    fn params_must_be_a_json_object() {
        let err = parse_params("[1,2]").unwrap_err();
        assert!(err.0.contains("JSON 对象"), "实得：{err:?}");
        let bad = parse_params("{oops").unwrap_err();
        assert!(bad.0.contains("不是合法 JSON"), "实得：{bad:?}");
    }
}
