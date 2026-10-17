use super::{
    json::{as_str, err, field, obj},
    DefError,
};
use rpx_core::reader::ObjValue;
use serde_json::json;
use serde_json::Value as J;

pub fn decode(v: &J, path: &str) -> Result<ObjValue, DefError> {
    let o = obj(v, path)?;
    let t = as_str(field(o, "type", path)?, path)?;
    let v = field(o, "value", path)?;
    let bad = || err(path, format!("无效的 {t} 字面量"));
    let integer = || {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            .ok_or_else(bad)
    };
    Ok(match t {
        "null" if v.is_null() => ObjValue::Null,
        "text" => ObjValue::Str(v.as_str().ok_or_else(bad)?.into()),
        "bool" => ObjValue::Bool(v.as_bool().ok_or_else(bad)?),
        "byte" => ObjValue::Byte(i8::try_from(integer()?).map_err(|_| bad())?),
        "short" => ObjValue::Short(i16::try_from(integer()?).map_err(|_| bad())?),
        "int" => ObjValue::Int(i32::try_from(integer()?).map_err(|_| bad())?),
        "long" => ObjValue::Long(integer()?),
        "float" => {
            let n = v.as_f64().ok_or_else(bad)? as f32;
            if !n.is_finite() {
                return Err(bad());
            }
            ObjValue::Float(n)
        }
        "double" => ObjValue::Double(v.as_f64().ok_or_else(bad)?),
        "date" => ObjValue::Millis {
            tag: 3,
            millis: integer()?,
        },
        "decimal" => {
            let s = v.as_str().ok_or_else(bad)?;
            let s = s.strip_prefix('-').unwrap_or(s);
            let (mantissa, exponent) = match s.split_once(['e', 'E']) {
                Some((m, e)) => (m, e.parse::<i64>().map_err(|_| bad())?),
                None => (s, 0),
            };
            let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
            if whole.is_empty()
                || !whole
                    .bytes()
                    .chain(fraction.bytes())
                    .all(|b| b.is_ascii_digit())
            {
                return Err(bad());
            }
            let mut bytes = vec![0u8];
            for c in whole.bytes().chain(fraction.bytes()) {
                let mut carry = (c - b'0') as u16;
                for b in bytes.iter_mut().rev() {
                    let n = *b as u16 * 10 + carry;
                    *b = n as u8;
                    carry = n >> 8;
                }
                if carry > 0 {
                    bytes.insert(0, carry as u8);
                }
            }
            if bytes[0] & 128 != 0 {
                bytes.insert(0, 0);
            }
            if v.as_str().unwrap().starts_with('-') {
                for b in &mut bytes {
                    *b = !*b;
                }
                for b in bytes.iter_mut().rev() {
                    let (n, c) = b.overflowing_add(1);
                    *b = n;
                    if !c {
                        break;
                    }
                }
            }
            ObjValue::BigDecimal {
                scale: i64::try_from(fraction.len())
                    .ok()
                    .and_then(|n| n.checked_sub(exponent))
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(bad)?,
                unscaled: bytes,
            }
        }
        _ => return Err(bad()),
    })
}
pub fn encode(v: &ObjValue) -> Option<J> {
    let (t, value) = match v {
        ObjValue::Null => ("null", J::Null),
        ObjValue::Str(v) => ("text", json!(v)),
        ObjValue::Bool(v) => ("bool", json!(v)),
        ObjValue::Byte(v) => ("byte", json!(v)),
        ObjValue::Short(v) => ("short", json!(v)),
        ObjValue::Int(v) => ("int", json!(v)),
        ObjValue::Long(v) => ("long", json!(v.to_string())),
        ObjValue::Float(v) => ("float", json!(v)),
        ObjValue::Double(v) => ("double", json!(v)),
        ObjValue::BigDecimal { scale, unscaled } => (
            "decimal",
            json!(rpx_engine::decimal_to_string(*scale, unscaled)),
        ),
        ObjValue::Millis { tag: 3, millis } => ("date", json!(millis.to_string())),
        _ => return None,
    };
    Some(json!({"type":t,"value":value}))
}
