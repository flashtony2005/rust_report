//! 字面量取值：模板格的 [`ObjValue`] → 计算态 [`Value`]。
//!
//! 这是阶段 B `prepareCalculate` 的**无表达式分支**（`ExtNormalCell.java:539-553`）：
//!
//! # 两段：先原样搬，再只对字符串"猜"一次类型
//!
//! ```java
//! if (this.engine.expression != null) { this.setExtensible((byte)17); }
//! else {
//!     this.setValue(this.engine.srcPrepare.getValue());
//!     if (this.cellValue != null && type == -64) {
//!         if (this.cellValue instanceof String) { this.cellValue = Variant2.parse(...); }
//!         if (dispValue == null && !isSpaceString(format)) { this.setDispValue(Variant2.format(...)); }
//!     }
//! }
//! ```
//!
//! - 第 1 段 [`literal_value`]：模板里写了什么类型，计算态原样拿到什么类型 ——
//!   **不做任何数值归一化**（M2-14/附件 H.3：`Integer(100)` 不得变成 `Double(100.0)`）。
//! - 第 2 段 [`parse_string`]：**只对 `String` 值**再解释一次（`"1000"` → `Integer(1000)`）。
//!   这是最容易被漏掉的一段，因为它**没有任何夹具钉住**（见该函数的文档）。
//! - 第 3 段（`dispValue` 的 `Variant2.format`）**未实现**：11 个网格夹具的 `format` 槽
//!   全为空（唯一设过 format 的 `Fixtures.java:221` 是 M1 的模板夹具），
//!   所以整条分支从未被触发。`disp_value` 目前只走"模板声明 + 继承"这条路径
//!   （`ExtNormalCell.getDispValue():1645-1651` 的 getter 回退语义）。
//!
//! # 唯一需要"算"的一支：`BigDecimal`
//!
//! `rpx_core` 按 M1 契约**只透传** `BigDecimal` 的 `(scale, unscaled)` 原始形态
//! （附件 A：标签 2 = int32 scale + 大端补码 unscaled 字节），不转十进制。
//! 而夹具的 `value` 段写的是 `o.toString()`（`CalcJson.java:233-235`），
//! 所以这里必须复刻 `java.math.BigDecimal.toString()`。
//!
//! 对应的夹具钉子：`engine_types` 的 `(4,1)` = `new BigDecimal("100.00")`
//! → JSON 原文 `"value":100.00`（**不是** `100.0`，那个"零"是精度信息）。
//!
//! # 未建模的四类（诚实边界）
//!
//! `BigInteger` / `Bytes` / `Millis`（时间）/ `Obj`（多态对象）在 [`Value`] 里
//! **没有对应变体**，且**没有任何夹具**钉住它们的计算态形态（`engine_types` 只覆盖
//! 9 类字面量）。这里返回 `None` 让调用方计数（[`crate::stage_b::StageB`]
//! 的 `unmodeled_literals`），**不静默降级成 `Null` 假装成功** —— 后者会让
//! 「日期字面量被当成空格」这类错误一路飘到 golden 才暴露，且看不出原因。

use rpx_core::reader::ObjValue;

use crate::value::Value;

mod decimal;

pub use decimal::decimal_to_string;

/// `INormalCell.TYPE_NORMAL`（`INormalCell.java:21`）。
pub const NORMAL_CELL_TYPE: i8 = -64;
/// `INormalCell.TYPE_HTML`（`INormalCell.java:25`）。
pub const HTML_CELL_TYPE: i8 = -58;
/// `INormalCell.TYPE_RICHTEXT`（`INormalCell.java:29`）。
pub const RICHTEXT_CELL_TYPE: i8 = -56;

/// 该格的值是否会被 `Variant2.parse` 二次解释 —— **两条 `prepareCalculate` 的守卫不一样**。
///
/// | 路径 | 源码 | 守卫 |
/// |------|------|------|
/// | `ExtNormalCell`（无表达式、非扩展格） | `ExtNormalCell.java:543` | `cellValue != null && type == -64` |
/// | `ExtExtendCell`（无表达式的**扩展格**） | `ExtExtendCell.java:333` | `cellValue != null && type == -64 \|\| type == -58 \|\| type == -56` |
///
/// ⛔ 第二行的 `&&` 优先于 `\|\|`，实为 `(cellValue != null && type == -64) \|\| type == -58 \|\| type == -56`
/// —— 后两项**不要求 `cellValue` 非空**（产品自己写出来的优先级别扭）。
/// 但真正做解释的仍是 `cellValue instanceof String`（`:335`，`null` 不是 `String`），
/// 所以两条路径合起来可约化为「值是字符串 **且** 类型命中」：
///
/// - 普通格：只有 `-64`
/// - 扩展格：`-64` / `-58`（HTML）/ `-56`（富文本）
///
/// ⚠ 现有夹具**没有** `-58`/`-56` 的格，也没有「声明 18/19 且无表达式」的格，
/// 所以这两条路径的差别**没有任何测试钉住**（见本项目"忠实实现 + 明确声明"的约定）。
pub fn parses_string_literal(cell_type: i8, is_extender: bool) -> bool {
    cell_type == NORMAL_CELL_TYPE
        || (is_extender && matches!(cell_type, HTML_CELL_TYPE | RICHTEXT_CELL_TYPE))
}

/// 模板值 → 计算态值。未建模的变体返回 `None`（见模块文档末段）。
pub fn literal_value(o: &ObjValue) -> Option<Value> {
    Some(match o {
        ObjValue::Null => Value::Null,
        ObjValue::Str(s) => Value::Text(s.clone()),
        ObjValue::Int(v) => Value::Int(*v),
        ObjValue::Long(v) => Value::Long(*v),
        ObjValue::Double(v) => Value::Double(*v),
        ObjValue::Float(v) => Value::Float(*v),
        ObjValue::Bool(v) => Value::Bool(*v),
        ObjValue::Byte(v) => Value::Byte(*v),
        ObjValue::Short(v) => Value::Short(*v),
        ObjValue::BigDecimal { scale, unscaled } => {
            Value::Decimal(decimal_to_string(*scale, unscaled))
        }
        ObjValue::BigInteger(_)
        | ObjValue::Bytes(_)
        | ObjValue::Millis { .. }
        | ObjValue::Obj { .. } => return None,
    })
}

/// `Variant2.parse(text, false)` 的复刻（`Variant2.java:752-801`）—— 字面量分支
/// 的**第二段**：模板值是字符串时，产品还会再"猜"一次它的类型。
///
/// ```java
/// if (this.cellValue instanceof String) { this.cellValue = Variant2.parse((String)this.cellValue, false); }
/// ```
///
/// 逐分支对齐（顺序即优先级）：
///
/// | 输入 | 结果 | 出处 |
/// |------|------|------|
/// | `""` | `null` | `:753-755` |
/// | 全空白 | **原文**（未 trim） | `:757-760` |
/// | 首字符是 `"` 或 `'` | **原文**（`removeEscAndQuote=false` ⇒ 不脱引号） | `:762-767` |
/// | `null`（忽略大小写） | `null` | `:768-770` |
/// | `true` / `false`（忽略大小写） | `Boolean` | `:771-776` |
/// | `0x…` / `0X…`（长度 > 2） | `Long`（十六进制；解析失败继续往下） | `:777-784` |
/// | 能当 `int` 解析 | `Integer` | `:786` |
/// | 能当 `long` 解析 | `Long` | `:790` |
/// | 能当 `double` 解析 | `Double` | `:794` |
/// | 其余 | 落到 `parseDate` | `:797` |
///
/// # 为什么必须做（夹具抓不到）
///
/// 现有引擎夹具的字符串字面量全是 `"region"` / `"North"` / `"int"` 这类**非数值**文本
/// （实测：12 个 `engine_*.expected.json` 的引号值里 0 个是数字），所以**漏掉整段
/// `parse` 也不会有任何测试失败**。但一旦某天来一个字面量 `"1000"`，
/// 产品的计算态是 `Integer(1000)`（`type=int`），漏实现则是 `string` —— 属可构造的真实分叉。
/// 与 [`crate::extend::EXTEND_EXCLUDED_CELL_TYPES`] 同一类：忠实实现 + 明确声明无夹具钉住。
///
/// # ⛔ 已声明缺口：`parseDate`
///
/// Java 的最后一道兜底是 `parseDate(text)`（`:797`），它有自己的日期语言
/// （`DateFactory2.parseTimestamp` / `parseDate` / `parseTime`），成功时返回
/// **`java.util.Date`**；失败则原样返回入参（`:810`/`:817`/`:825`/`:832`）。
/// 本 crate **未实现日期语言**，一律走"原样返回"分支 —— 于是形如 `2024-01-02`
/// 的字符串字面量在这里是 `Text`，产品却是 `Date`。
/// 之所以敢这么收口：`Value` 本来就没有日期变体，这个缺口**无论是否实现 parseDate
/// 都必须声明**（见模块文档末段的四类未建模变体）。
///
/// # 另一类已知差异（都**无夹具涉及**）
///
/// 列出以免被当成"已核实等价"：
///
/// | 输入 | Java | 本实现 | 原因 |
/// |------|------|--------|------|
/// | `1.5f` / `1.5d` | `Double(1.5)` | `Text` | Java 的 `Double.valueOf` 接受类型尾缀，Rust `f64::from_str` 不接受 |
/// | `0x1.8p1` | `Double(3.0)` | `Text` | Java 还接受**十六进制浮点**字面量 |
/// | 含非 ASCII 且长度恰为 2 的串 | 长度按 UTF-16 码元算 | 长度按**字节**算（`s.len() > 2`） | 只影响 `0x` 前缀判断的边界；`"0x"` 这类串两种情况都不够长，结论一致 |
pub fn parse_string(text: &str) -> Value {
    if text.is_empty() {
        return Value::Null;
    }
    let s = text.trim();
    if s.is_empty() {
        return Value::Text(text.to_string());
    }
    let first = s.chars().next().unwrap_or('\0');
    if first == '"' || first == '\'' {
        return Value::Text(text.to_string());
    }
    if s.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if s.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if s.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if s.len() > 2 && (s.starts_with("0x") || s.starts_with("0X")) {
        if let Ok(v) = i64::from_str_radix(&s[2..], 16) {
            return Value::Long(v);
        }
    }
    if let Ok(v) = s.parse::<i32>() {
        return Value::Int(v);
    }
    if let Ok(v) = s.parse::<i64>() {
        return Value::Long(v);
    }
    if let Ok(v) = s.parse::<f64>() {
        return Value::Double(v);
    }
    Value::Text(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_tags_map_one_to_one_without_widening() {
        // 附件 H.3：类型必须原样保留，Integer(100) 不得变成 Double(100.0)
        assert_eq!(literal_value(&ObjValue::Int(100)), Some(Value::Int(100)));
        assert_eq!(literal_value(&ObjValue::Long(7)), Some(Value::Long(7)));
        assert_eq!(literal_value(&ObjValue::Short(7)), Some(Value::Short(7)));
        assert_eq!(literal_value(&ObjValue::Byte(7)), Some(Value::Byte(7)));
        assert_eq!(literal_value(&ObjValue::Double(100.0)), Some(Value::Double(100.0)));
        assert_eq!(literal_value(&ObjValue::Float(1.5)), Some(Value::Float(1.5)));
        assert_ne!(literal_value(&ObjValue::Int(100)), literal_value(&ObjValue::Double(100.0)));
    }

    #[test]
    fn string_and_boolean_and_null_keep_their_identity() {
        assert_eq!(literal_value(&ObjValue::Str("int".into())), Some(Value::Text("int".into())));
        assert_eq!(literal_value(&ObjValue::Bool(true)), Some(Value::Bool(true)));
        // ⛔ 空格是 Null，不是空串（Value::Null ≠ Value::Text("")）
        assert_eq!(literal_value(&ObjValue::Null), Some(Value::Null));
        assert_ne!(literal_value(&ObjValue::Null), literal_value(&ObjValue::Str(String::new())));
    }

    #[test]
    fn unmodeled_variants_report_none_instead_of_silently_becoming_null() {
        assert_eq!(literal_value(&ObjValue::BigInteger(vec![1])), None);
        assert_eq!(literal_value(&ObjValue::Bytes(vec![1, 2])), None);
        assert_eq!(literal_value(&ObjValue::Millis { tag: 3, millis: 0 }), None);
        assert_eq!(literal_value(&ObjValue::Obj { class_name: None, raw: vec![] }), None);
    }

    #[test]
    fn parse_string_is_gated_by_cell_type_and_by_the_extender_role() {
        // 两条 `prepareCalculate` 的守卫合成后是三行真值表（见函数文档）：
        //   -64（普通格）：两条路径都做二次解释 ⇒ 与 is_extender 无关
        assert!(parses_string_literal(NORMAL_CELL_TYPE, false));
        assert!(parses_string_literal(NORMAL_CELL_TYPE, true));
        // -58（HTML）/ -56（富文本）：**只有扩展格**那条路径才有（`ExtExtendCell.java:333`）
        assert!(!parses_string_literal(HTML_CELL_TYPE, false));
        assert!(parses_string_literal(HTML_CELL_TYPE, true));
        assert!(!parses_string_literal(RICHTEXT_CELL_TYPE, false));
        assert!(parses_string_literal(RICHTEXT_CELL_TYPE, true));
        // ⛔ 图形 / 子报表 / 条形码：`checkExtend` 开头三条短路（`ExtNormalCell.java:494-496`）
        // 让它们永远是普通格 ⇒ 即便被当成扩展格也**不**走 -58/-56 分支。
        // 杀掉"照抄 Java 字面量 `(非空 && -64) || -58 || -56`、忽略合并同类项"的退化翻译
        // （那种写法会在这里给 true）。
        for t in [-61i8, -60, -57] {
            assert!(!parses_string_literal(t, false), "类型 {t} 不该 parse");
            assert!(!parses_string_literal(t, true), "类型 {t} 即便被判成扩展格也不该 parse");
        }
        // 其余类型（含 16/17/18/19 这些扩展方式码）一律不 parse
        for t in [0i8, 16, 17, 18, 19] {
            assert!(!parses_string_literal(t, false));
            assert!(!parses_string_literal(t, true));
        }
    }

    #[test]
    fn parse_string_follows_the_numeric_ladder_int_then_long_then_double() {
        // 顺序即优先级：`100` 必须是 Integer，不是 Double（M2-14）
        assert_eq!(parse_string("100"), Value::Int(100));
        assert_eq!(parse_string("  100  "), Value::Int(100)); // 用 trim 后的串解析
        assert_eq!(parse_string("1234567890123"), Value::Long(1234567890123));
        assert_eq!(parse_string("92.5"), Value::Double(92.5));
        assert_eq!(parse_string("1e5"), Value::Double(100000.0));
        // int 溢出 → long，不得退化成 double
        assert_ne!(parse_string("3000000000"), Value::Double(3000000000.0));
        assert_eq!(parse_string("3000000000"), Value::Long(3000000000));
    }

    #[test]
    fn parse_string_handles_null_bool_and_hex_before_the_ladder() {
        assert_eq!(parse_string("null"), Value::Null);
        assert_eq!(parse_string("NULL"), Value::Null);
        assert_eq!(parse_string("true"), Value::Bool(true));
        assert_eq!(parse_string("FALSE"), Value::Bool(false));
        assert_eq!(parse_string("0xFF"), Value::Long(255));
    }

    #[test]
    fn parse_string_keeps_non_numeric_text_verbatim_including_whitespace() {
        // 落到 `parseDate` 的兜底：失败时返回**未 trim** 的入参（`:832`）
        assert_eq!(parse_string("North"), Value::Text("North".into()));
        assert_eq!(parse_string("  North  "), Value::Text("  North  ".into()));
        assert_eq!(parse_string("int"), Value::Text("int".into()));
        // 首字符是引号 → 原样（removeEscAndQuote=false，不脱引号）
        assert_eq!(parse_string("\"x\""), Value::Text("\"x\"".into()));
        // 空串与全空白是两回事：前者 null，后者原文
        assert_eq!(parse_string(""), Value::Null);
        assert_eq!(parse_string("   "), Value::Text("   ".into()));
    }

    #[test]
    fn parse_string_declares_the_parse_date_gap() {
        // ⛔ 已声明的缺口：日期语言未实现，`2024-01-02` 在这里是 Text，
        // 产品会给 java.util.Date。这条测试**故意钉住缺口**（而非钉住正确行为）：
        // 实现 parseDate 时必须同步改这里。
        assert_eq!(parse_string("2024-01-02"), Value::Text("2024-01-02".into()));
    }
}
