//! 数值运算：`Value` 上的算术/比较/布尔原语（自 `expr/eval.rs` 拆出）。
//!
//! Java 对应：`Variant2.add/sub/mul/…` 系列静态方法。
//! 语义事实源：**附件 H.3（类型保真）** ——
//! ⛔ Int 不提升为 Double（Java 重载决议走 `int + int`），溢出按 Java 回绕
//! （`wrapping_*`）；`7/2` → `Double(3.5)`（`Variant2.divide` 全部走 double）；
//! Text + Text 是字符串拼接（`Variant2.add` 的 String 分支）。
//!
//! 算法正文见 `docs/m2-engine-design.md` 附件 H.3；单测在 `expr/tests_eval.rs`
//! 的 `int_type_fidelity_*` 一族。

use crate::error::{EngineError, Result};
use crate::value::Value;

/// 统一升到 `f64` 的通道（仅用于 mixed / div / cmp 路径）。
pub(crate) fn to_f64(v: &Value) -> Option<f64> {
    Some(match v {
        Value::Int(i) => *i as f64,
        Value::Long(l) => *l as f64,
        Value::Double(d) => *d,
        Value::Float(f) => *f as f64,
        Value::Short(s) => *s as f64,
        Value::Byte(b) => *b as f64,
        _ => return None,
    })
}

/// 加法：Text+Text 拼接 → Int/Long 同型回绕 → 其余升 Double。
pub(crate) fn num_add(a: &Value, b: &Value) -> Result<Value> {
    if let (Value::Text(x), Value::Text(y)) = (a, b) {
        return Ok(Value::Text(format!("{x}{y}"))); // Variant2.add 的字符串拼接
    }
    if let (Value::Int(x), Value::Int(y)) = (a, b) {
        return Ok(Value::Int(x.wrapping_add(*y))); // Java Integer 溢出回绕
    }
    if let (Value::Long(x), Value::Long(y)) = (a, b) {
        return Ok(Value::Long(x.wrapping_add(*y)));
    }
    match (to_f64(a), to_f64(b)) {
        (Some(x), Some(y)) => Ok(Value::Double(x + y)),
        _ => Err(EngineError::UnsupportedExpr {
            expr: format!("{a:?} + {b:?}"),
        }),
    }
}

/// 减法：Int/Long 同型回绕，其余升 Double。
pub(crate) fn num_sub(a: &Value, b: &Value) -> Result<Value> {
    if let (Value::Int(x), Value::Int(y)) = (a, b) {
        return Ok(Value::Int(x.wrapping_sub(*y)));
    }
    if let (Value::Long(x), Value::Long(y)) = (a, b) {
        return Ok(Value::Long(x.wrapping_sub(*y)));
    }
    match (to_f64(a), to_f64(b)) {
        (Some(x), Some(y)) => Ok(Value::Double(x - y)),
        _ => Err(EngineError::UnsupportedExpr {
            expr: format!("{a:?} - {b:?}"),
        }),
    }
}

/// 乘法：Int/Long 同型回绕，其余升 Double。
pub(crate) fn num_mul(a: &Value, b: &Value) -> Result<Value> {
    if let (Value::Int(x), Value::Int(y)) = (a, b) {
        return Ok(Value::Int(x.wrapping_mul(*y)));
    }
    if let (Value::Long(x), Value::Long(y)) = (a, b) {
        return Ok(Value::Long(x.wrapping_mul(*y)));
    }
    match (to_f64(a), to_f64(b)) {
        (Some(x), Some(y)) => Ok(Value::Double(x * y)),
        _ => Err(EngineError::UnsupportedExpr {
            expr: format!("{a:?} * {b:?}"),
        }),
    }
}

/// 除法：产品 `Variant2.divide` 无整型分支 ⇒ 全部升 Double（`7/2` → 3.5）。
pub(crate) fn num_div(a: &Value, b: &Value) -> Result<Value> {
    match (to_f64(a), to_f64(b)) {
        (Some(x), Some(y)) => Ok(Value::Double(x / y)),
        _ => Err(EngineError::UnsupportedExpr {
            expr: format!("{a:?} / {b:?}"),
        }),
    }
}

/// 取模：Int 同型保留（Java `int % int`），其余升 Double。
pub(crate) fn num_mod(a: &Value, b: &Value) -> Result<Value> {
    if let (Value::Int(x), Value::Int(y)) = (a, b) {
        return Ok(Value::Int(x % y));
    }
    match (to_f64(a), to_f64(b)) {
        (Some(x), Some(y)) => Ok(Value::Double(x % y)),
        _ => Err(EngineError::UnsupportedExpr {
            expr: format!("{a:?} % {b:?}"),
        }),
    }
}

/// 一元负号：Int/Long 回绕取负，其余升 Double 后取负。
pub(crate) fn num_neg(v: &Value) -> Result<Value> {
    Ok(match v {
        Value::Int(i) => Value::Int(i.wrapping_neg()),
        Value::Long(l) => Value::Long(l.wrapping_neg()),
        other => match to_f64(other) {
            Some(x) => Value::Double(-x),
            None => {
                return Err(EngineError::UnsupportedExpr {
                    expr: format!("-{other:?}"),
                })
            }
        },
    })
}

/// 数值比较（仅 GT/GE/LT/LE 路径；NaN 视为 Equal——夹具无 NaN 面）。
///
/// `Text-Text` 走产品字典序 [`crate::collation::compare_text`]（`Variant2.compare2:426`
/// 的 String 分支 → `ChineseComparator`），**不是** UTF-16/字典序 —— 二者会给出相反答案
/// （如 `"一"` vs `"丁"`，见 `docs/m2-collation.md` §4 与 `docs/m2-engine-design.md` §14）。
/// 该分支必须在取 `to_f64` **之前**判断，否则两个文本会落进"非数值 ⇒ 报错"。
pub(crate) fn num_cmp(a: &Value, b: &Value) -> Result<std::cmp::Ordering> {
    if let (Value::Text(x), Value::Text(y)) = (a, b) {
        return Ok(crate::collation::compare_text(x, y));
    }
    match (to_f64(a), to_f64(b)) {
        (Some(x), Some(y)) => Ok(x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)),
        _ => Err(EngineError::UnsupportedExpr {
            expr: format!("{a:?} <=> {b:?}"),
        }),
    }
}

/// 相等判断：数值面走 `f64` 比较（Java `Variant2.compareObjects` 的语义），
/// 非数值（Text/Bool/Null）退回结构相等。
///
/// ⛔ **`Text-Text` 走精确字符串相等，不经过 collator。** 依据 `Variant2.equals:469-470`：
/// 两个同类型对象先 `getSingleValue`，`String` 分支再用 `String.equals` —— 与
/// `ChineseComparator` 无关。故 `"\u{301}" == ""` 为 **false**（字符串不同），
/// 尽管 collation 下二者相等（`CE("\u{301}") == []`）。别把这里改成走 collation。
pub(crate) fn val_eq(a: &Value, b: &Value) -> bool {
    match (to_f64(a), to_f64(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// `max` / `min` 的聚合比较：`Variant2.compare2`（`:418-458`）的**可排序子集**。
///
/// 调用方负责解包：产品走 `Variant2.compare`（`:364-368`）＝
/// `compare2(getSingleValue(a), getSingleValue(b))`；本仓由调用点做 `single_value`
/// （`eval.rs::unwrap_single` / `dsfn.rs` 的 DS 极值）。
///
/// 与 [`num_cmp`] 的三点差别：
///
/// 1. **null 分支**（`compare2:419-424`，**先于**类型分派）：`(null,null)=0`、
///    `(null,x)=-1`、`(x,null)=1`。区域比较会走到它 —— `Max/Min._$1:159` 的
///    `Variant2.compare(result, obj)` 里 `obj` 是**原始对象**，元素是空 List 时
///    解包得 null（`Value::List(vec![])` ⇒ `single_value` → `None`）。
/// 2. 多一支 `Bool-Bool`（`compare2:453-455` → `compare(boolean,boolean):351-359`，
///    `false < true`）；
/// 3. `Text-Text` 走产品字典序 [`crate::collation::compare_text`]（`compare2:425-427` 的
///    String 分支 → `ChineseComparator`，即 `reportlite/lib/reportlite.jar!`
///    `/com/raqsoft/commonl/rule.txt` → JDK `RuleBasedCollator`）。复刻依据与坑见
///    `docs/m2-collation.md`。
///
/// ⚠ **未建模的同族缺口（已声明，勿当成"顺手统一"）**：[`num_cmp`]（`<`/`>`/
/// `<=`/`>=` 用）**没有** null 分支，而产品那些运算符同样走 `Variant2.compare`
/// （`operator/Smaller.java:40` 等）⇒ 表达式的操作数为 null 时本仓报
/// `UnsupportedExpr`、产品按上面 (1) 返回定义值。**无夹具**，故本轮不动它
/// （登记位置：`docs/m2-engine-design.md` §14.6 的缺口清单）。
///
/// ⚠ 本函数**继承** `to_f64` 的全部数值语义，即 `merge/tripwire.rs` 记录的
/// 7 处分歧在这里同样成立：`Decimal` 与数值混比会报错（分歧 A 的后果）、
/// `Double`/`Float` 的 ±0 与 NaN、`Long` 超 2^53 都会偏离 Java。
/// **修 `to_f64` 时必须连这些一起重核。**
pub(crate) fn aggregate_cmp(a: &Value, b: &Value) -> Result<std::cmp::Ordering> {
    match (a, b) {
        // `compare2:419-424`：null 判定在**最前面**，先于 String/Number/Date/Bool 分派。
        (Value::Null, Value::Null) => Ok(std::cmp::Ordering::Equal),
        (Value::Null, _) => Ok(std::cmp::Ordering::Less),
        (_, Value::Null) => Ok(std::cmp::Ordering::Greater),
        (Value::Bool(x), Value::Bool(y)) => Ok(x.cmp(y)),
        (Value::Text(x), Value::Text(y)) => Ok(crate::collation::compare_text(x, y)),
        _ => num_cmp(a, b),
    }
}

/// 布尔强转：只有 `Bool` 通过，其余报错（产品无隐式真值转换）。
pub(crate) fn as_bool(v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(EngineError::UnsupportedExpr {
            expr: format!("非布尔: {other:?}"),
        }),
    }
}
