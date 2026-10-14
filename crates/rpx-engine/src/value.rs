//! 计算态的值类型。
//!
//! **类型即契约**（附件 H.3）：Java 侧 `Integer(100)` 必须产出 `int`，不能统一提升成
//! `f64`/`double`，否则 `expected.json` 里的 `type` 会全表 FAIL。因此这里**不做数值归一化**，
//! 每个变体各占一格，`type_name()` 与夹具落盘值一一对应。
//!
//! `Decimal` 承载 `java.math.BigDecimal` 的**规范十进制字面量**：本 crate 是零依赖，
//! 不引 bigdecimal 库，而把精度丢掉换成 f64 恰好是契约禁止的事 —— 保留原字面量是
//! 唯一既零依赖又不失真（也不撒谎）的表示。
//!
//! 对应 Java 侧 `CalcJson.typeName` 的取值集合，逐字对齐（探针 `CalcJson.java:224-253`
//! 的九个显式分支：null/string/boolean/int/long/decimal/double/float/list）。
//!
//! ⚠ **兜底分支（已按证据补齐）**：Java 的 `typeName` 末尾还有一条
//! `return o.getClass().getSimpleName();`（`CalcJson.java:252`），会把 `Short`/`Byte`
//! 这类值产出成**首字母大写的类名**（`"Short"`/`"Byte"`），而同一支 `value()` 又把它们
//! 当数值渲染。原先因"引擎是否已把它们归一成 `Integer` 无基准"而**故意不加**这两个变体；
//! 现已有证据（`ProbeTypes` 探针实测，2026-10-12）：`Short`/`Byte` 字面量能穿过
//! `ReportUtils.write → read → 计算` 全链路而不降级，且落到上述兜底名 —— 故补上变体，
//! 使枚举对 `CalcJson.typeName` 的输出**封闭**。
//!
//! 承载它们的夹具是第 8 个引擎夹具 `engine_types`（模板态含各类型字面量）；
//! 在它落地前，`"Short"`/`"Byte"` 两个名字尚无**夹具级**断言，只有探针级证据。

use std::fmt;

/// 计算态的一个值。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// 空格（Java `null`）。夹具里是 JSON `null`。
    Null,
    /// `java.lang.Integer`
    Int(i32),
    /// `java.lang.Long`
    Long(i64),
    /// `java.lang.Double`
    Double(f64),
    /// `java.lang.Float` —— 与 `Double` **分属两个变体**：`CalcJson.typeName` 有独立的
    /// `float` 分支，`rpx_core::ObjValue` 也单列了 `Float(f32)`，合并即违约。
    Float(f32),
    /// `java.lang.Short` —— 走 `CalcJson.typeName` 的**兜底分支**，落盘名是 `"Short"`
    /// （首字母大写，与上面八个小写名不同，这是产品探针的真实行为，不是笔误）。
    Short(i16),
    /// `java.lang.Byte` —— 同上，落盘名 `"Byte"`。
    Byte(i8),
    /// `java.math.BigDecimal`，存规范十进制字面量（见模块文档）。
    Decimal(String),
    /// `java.lang.String`
    Text(String),
    /// `java.lang.Boolean`
    Bool(bool),
    /// `java.util.List` —— 聚合语义的开关（附件 H.1：只有操作数求值为 List 才聚合）。
    List(Vec<Value>),
}

impl Value {
    /// 与 `expected.json` 的 `type` 字段逐字对齐的短名。
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Int(_) => "int",
            Value::Long(_) => "long",
            Value::Double(_) => "double",
            Value::Float(_) => "float",
            Value::Short(_) => "Short",
            Value::Byte(_) => "Byte",
            Value::Decimal(_) => "decimal",
            Value::Text(_) => "string",
            Value::Bool(_) => "boolean",
            Value::List(_) => "list",
        }
    }

    /// 是否是数值型（聚合、算术的候选）。`Bool`/`Text`/`List`/`Null` 不算。
    /// `Short`/`Byte` 是数值（Java 的 `value()` 也把它们当数值渲染），只是类型名走兜底分支。
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Value::Int(_)
                | Value::Long(_)
                | Value::Double(_)
                | Value::Float(_)
                | Value::Short(_)
                | Value::Byte(_)
                | Value::Decimal(_)
        )
    }

    /// 算术前把整型提升到 i64，用于整数间的加乘（保持整数不溢出到浮点）。
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(i64::from(*v)),
            Value::Long(v) => Some(*v),
            Value::Short(v) => Some(i64::from(*v)),
            Value::Byte(v) => Some(i64::from(*v)),
            _ => None,
        }
    }

    /// 是否为空。注意 `Null` 与 `Text("")` 是两回事。
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

/// `Variant2.getSingleValue`（`Variant2.java:85-94`）：**空 `List` → `null`、
/// 非空 `List` → 首元素、其余原样**。
///
/// 产品里所有"聚合一个 List"的函数（`Sum` / `Average` / `Max` / `Min` /
/// `Variant2.equals`）都先过这一层，所以它必须是**全仓唯一一份** ——
/// 两处并行声明迟早漂移（本仓 `silent-failure-hunt` 记过的老毛病）。
/// 2026-10-13 收敛：此前 `merge.rs` 有一份私有副本，`expr/dsfn.rs` 的
/// `sum_list` 则**完全没过这一层**（见那边的 §M2-12 注记）。
///
/// ⚠ 与 Java 的一处**已声明近似**：Java 的 `getSingleValue` 会先过
/// `Variant2.getValue` 拆 `Variable` / `DispValue` 包装（`:86`），而本 crate
/// 的值已是裸值，无需拆。
pub(crate) fn single_value(v: &Value) -> Option<&Value> {
    match v {
        Value::Null => None,
        Value::List(items) => items.first(),
        other => Some(other),
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, ""),
            Value::Int(v) => write!(f, "{v}"),
            Value::Long(v) => write!(f, "{v}"),
            Value::Double(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Short(v) => write!(f, "{v}"),
            Value::Byte(v) => write!(f, "{v}"),
            Value::Decimal(v) => write!(f, "{v}"),
            Value::Text(v) => write!(f, "{v}"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{it}")?;
                }
                write!(f, "]")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_names_match_the_fixture_vocabulary() {
        // 逐字对齐 CalcJson.typeName —— 改这里等于改 golden 的判定标准
        assert_eq!(Value::Null.type_name(), "null");
        assert_eq!(Value::Int(100).type_name(), "int");
        assert_eq!(Value::Long(100).type_name(), "long");
        assert_eq!(Value::Double(100.0).type_name(), "double");
        assert_eq!(Value::Float(100.0).type_name(), "float");
        assert_eq!(Value::Decimal("100.00".into()).type_name(), "decimal");
        assert_eq!(Value::Text("North".into()).type_name(), "string");
        assert_eq!(Value::Bool(true).type_name(), "boolean");
        assert_eq!(Value::List(vec![Value::Int(1)]).type_name(), "list");
        // ⚠ 这两个走 CalcJson.typeName 的**兜底分支**（o.getClass().getSimpleName()）：
        // 名字首字母大写，与上面九个小写名不同 —— 这是探针实测的产品行为，别"纠正"成 int。
        assert_eq!(Value::Short(7).type_name(), "Short");
        assert_eq!(Value::Byte(7).type_name(), "Byte");
    }

    #[test]
    fn short_and_byte_are_numeric_but_keep_their_own_type_name() {
        // 类型名走兜底、数值语义不变：Java 的 value() 也把它们当数值渲染（CalcJson.java:197-200）。
        assert!(Value::Short(7).is_numeric());
        assert!(Value::Byte(7).is_numeric());
        assert_eq!(Value::Short(7).as_i64(), Some(7));
        assert_eq!(Value::Byte(7).as_i64(), Some(7));
        // 不降级成 Int —— M2-14 禁止数值归一化
        assert_ne!(Value::Short(7).type_name(), Value::Int(7).type_name());
        assert_ne!(Value::Byte(7), Value::Int(7));
    }

    #[test]
    fn no_numeric_unification_across_variants() {
        // M2-14 的核心：类型不同就是不同值，"都是 100" 不能互相顶替
        assert_ne!(Value::Int(100), Value::Double(100.0));
        assert_ne!(Value::Double(100.0), Value::Float(100.0));
        assert_ne!(Value::Float(100.0), Value::Decimal("100".into()));
        assert_ne!(Value::Int(100), Value::Long(100));
        assert_ne!(Value::Int(100), Value::Decimal("100".into()));
        assert_eq!(Value::Int(100), Value::Int(100));
    }

    #[test]
    fn null_is_not_the_empty_string() {
        assert_ne!(Value::Null, Value::Text(String::new()));
        assert!(Value::Null.is_null());
    }

    #[test]
    fn list_display_is_java_like_which_is_how_the_engine_prints_sub_values() {
        let list = Value::List(vec![Value::Text("R1".into()), Value::Text("R2".into())]);
        assert_eq!(list.to_string(), "[R1,R2]");
    }

    #[test]
    fn integer_promotion_stays_integral() {
        assert_eq!(Value::Int(7).as_i64(), Some(7));
        assert_eq!(Value::Long(7).as_i64(), Some(7));
        assert_eq!(Value::Double(7.0).as_i64(), None);
        assert!(Value::Int(1).is_numeric());
        assert!(!Value::List(vec![]).is_numeric());
    }
}
