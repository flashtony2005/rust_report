use crate::error::{Error, Result};
use rpx_engine::{ParamTable, Value};
use serde_json::Value as J;

pub fn parse(j: &J) -> Result<ParamTable> {
    let map = j
        .as_object()
        .ok_or_else(|| Error::new(4, "PARAM_INVALID", "parameters", "参数必须是 JSON 对象"))?;
    map.iter()
        .map(|(k, v)| {
            Ok((
                k.clone(),
                convert(v, &format!("/{}", k.replace('~', "~0").replace('/', "~1")))?,
            ))
        })
        .collect()
}
fn convert(j: &J, path: &str) -> Result<Value> {
    Ok(match j {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(*b),
        J::String(s) => Value::Text(s.clone()),
        J::Number(n) => {
            if let Some(i) = n.as_i64() {
                i32::try_from(i).map(Value::Int).unwrap_or(Value::Long(i))
            } else if n.is_u64() {
                return Err(
                    Error::new(4, "PARAM_OVERFLOW", "parameters", "整数超出 i64 范围").at(path),
                );
            } else {
                Value::Double(n.as_f64().ok_or_else(|| {
                    Error::new(4, "PARAM_INVALID", "parameters", "无效数字").at(path)
                })?)
            }
        }
        J::Array(a) => Value::List(
            a.iter()
                .enumerate()
                .map(|(i, v)| convert(v, &format!("{path}/{i}")))
                .collect::<Result<_>>()?,
        ),
        J::Object(_) => {
            return Err(
                Error::new(4, "PARAM_INVALID", "parameters", "参数值不支持对象形态").at(path),
            )
        }
    })
}
