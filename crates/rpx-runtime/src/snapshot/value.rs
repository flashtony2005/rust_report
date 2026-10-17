//! `Value` → JSON 值 + `CellValueType`（映射表：`docs/spec-m4.md` §4.3.1）。

use serde_json::{json, Value as J};

use rpx_engine::value::Value;

/// Univer `CellValueType`：`STRING=1 / NUMBER=2 / BOOLEAN=3 / FORCE_STRING=4`。
/// 文本一律 `FORCE_STRING`：防 `"00123"` 这类**文本数字**被 Univer 推断成数字
/// （保真：来源就是字符串）。
pub(crate) const T_FORCE_STRING: u8 = 4;
const T_NUMBER: u8 = 2;
const T_BOOLEAN: u8 = 3;

/// 一个值 → `(JSON v, 可选 t)`。
///
/// * `Decimal` 转 number：**丢尾零是声明近似**（显示对齐优先；数字格式 `n`
///   后置补，`spec-m4.md` §7）；超出 f64 的极端形态退字符串，不吞。
/// * `NaN`/`Inf` → `null`（JSON 无此表达；显式落 null 而不是静默成 0）。
/// * `List` → `"a,b,c"`（v1 近似，同 §7）。
pub(crate) fn value_json(v: &Value) -> (J, Option<u8>) {
    match v {
        Value::Null => (J::Null, None),
        Value::Int(n) => (json!(n), Some(T_NUMBER)),
        Value::Long(n) => (json!(n), Some(T_NUMBER)),
        Value::Short(n) => (json!(n), Some(T_NUMBER)),
        Value::Byte(n) => (json!(n), Some(T_NUMBER)),
        Value::Double(f) => num_json(*f),
        Value::Float(f) => num_json(f64::from(*f)),
        Value::Decimal(s) => match s.parse::<f64>() {
            Ok(f) => num_json(f),
            Err(_) => (J::String(s.clone()), Some(T_FORCE_STRING)),
        },
        Value::Text(s) => (J::String(s.clone()), Some(T_FORCE_STRING)),
        Value::Bool(b) => (json!(b), Some(T_BOOLEAN)),
        Value::List(items) => (J::String(join_list(items)), Some(T_FORCE_STRING)),
    }
}

/// 非有限数 → `null`（`spec-m4.md` §7 的 NaN/Inf 条目）。
fn num_json(f: f64) -> (J, Option<u8>) {
    if f.is_finite() {
        (json!(f), Some(T_NUMBER))
    } else {
        (J::Null, None)
    }
}

/// `List` 的 v1 近似：逗号连接（嵌套 List 递归成 `[..]`）。
fn join_list(items: &[Value]) -> String {
    let parts: Vec<String> = items.iter().map(scalar_text).collect();
    parts.join(",")
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Int(n) => n.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Short(n) => n.to_string(),
        Value::Byte(n) => n.to_string(),
        Value::Double(f) => f.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Decimal(s) => s.clone(),
        Value::Text(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::List(inner) => format!("[{}]", join_list(inner)),
    }
}
