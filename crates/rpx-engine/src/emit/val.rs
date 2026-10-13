//! `CalcJson.value(Object)` 与 `String.valueOf(Object)` 的逐分支镜像。
//!
//! # 两个函数，别混用
//!
//! | 本模块 | Java 对应 | 用在哪 |
//! |--------|-----------|--------|
//! | [`render_value`] | `CalcJson.value`（`CalcJson.java:219-254`） | `value` / `sub_values[].value` |
//! | [`java_tostring`] | `String.valueOf(v)` = `v.toString()` | **只**用于 `disp_value` 的判定（`CalcJson.java:71`） |
//!
//! 两者**不等价**，这正是一个易踩的坑：`value()` 会把整数值的 `Double` 折叠成裸整数
//! （`CalcJson.java:241-243` 的 `d == Math.floor(d) && Math.abs(d) < 1e15`），
//! 而 `String.valueOf` 直接调 `Double.toString` ⇒ 对 `Double(100.0)` 得 `"100.0"`。
//! 于是 `disp_value == "100.0"` 的格**不会**写 `disp_value`（判等成立），
//! 尽管它的 `value` 落在 JSON 里是 `100`。夹具 `engine_types` 的 `(2,4)` 就是
//! `value:100` / `type:"double"`，用它对照即可看清这条区别。

use crate::value::Value;

use super::json;
use super::num;

/// `CalcJson.value(Object)`：把计算态值渲染成 JSON 片段（**不是**字符串字面量）。
///
/// 分支顺序照抄 `CalcJson.java:219-254`：
/// `null` → 原样；`String` → 转义引号；`Boolean` → `true`/`false`；
/// `Integer/Long/Short/Byte` → 裸十进制；`BigDecimal` → **原样字面量**
/// （本 crate 把 `BigDecimal.toString` 的规范字面量存在 [`Value::Decimal`] 里，
/// 所以这里只搬不改，见 `crate::literal::decimal_to_string`）；
/// `Double/Float` → 见 [`render_float`]；`List` → JSON 数组逐项递归。
pub fn render_value(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Text(s) => json::quote(s),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Long(i) => i.to_string(),
        Value::Short(i) => i.to_string(),
        Value::Byte(i) => i.to_string(),
        Value::Decimal(s) => s.clone(),
        Value::Double(d) => render_float(*d, None),
        Value::Float(f) => render_float(f64::from(*f), Some(*f)),
        Value::List(items) => json::arr(items.iter().map(render_value).collect()),
    }
}

/// `CalcJson.java:236-245` 的 `Double || Float` 分支。
///
/// ⛔ 三个子分支的顺序不能换：
/// 1. `Double.isNaN(d) || Double.isInfinite(d)` → **带引号的字符串**
///    （`Json.quote(String.valueOf(d))`，即 `"NaN"` / `"Infinity"` / `"-Infinity"`）。
///    ⚠ 这里 `d` 恒是 `doubleValue()` 的结果 —— 即便原值是 `Float`，
///    `String.valueOf(d)` 拼的也是 **double** 的写法，不是 `Float.toString`。
/// 2. `d == Math.floor(d) && Math.abs(d) < 1e15` → `Long.toString((long)d)`（裸整数）。
/// 3. 其余 → `o.toString()`：**`Double` 用 `Double.toString`，`Float` 用 `Float.toString`**
///    —— 故 `orig` 保存着原始 `f32`，非整数分支必须用它而不是 `f64`。
fn render_float(d: f64, orig: Option<f32>) -> String {
    if d.is_nan() || d.is_infinite() {
        return json::quote(&num::java_double_to_string(d));
    }
    if d == d.floor() && d.abs() < 1e15 {
        // `(long) d` 向零截断；此处 d 已是整数值，无精度损失
        return (d as i64).to_string();
    }
    match orig {
        Some(f) => num::java_float_to_string(f),
        None => num::java_double_to_string(d),
    }
}

/// `String.valueOf(v)` / `v.toString()` —— **只**给 `disp_value` 的判等用。
///
/// ⚠ `List` 那一支是**推断**：Java 的 `List.toString`（`AbstractCollection.toString`）
/// 写作 `[a, b]`（逗号后**带一个空格**），而 `Value::Display` 写的是 `[a,b]`。
/// 两者不可能同时正确，故这里按 `AbstractCollection.toString` 的写法，且**声明没有夹具
/// 钉住**（12 个夹具里 `disp_value` 的 7 个值全是字符串，没有一个来自 `List`）。
pub fn java_tostring(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Text(s) => s.clone(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Long(i) => i.to_string(),
        Value::Short(i) => i.to_string(),
        Value::Byte(i) => i.to_string(),
        Value::Decimal(s) => s.clone(),
        Value::Double(d) => num::java_double_to_string(*d),
        Value::Float(f) => num::java_float_to_string(*f),
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(java_tostring).collect();
            format!("[{}]", parts.join(", "))
        }
    }
}
