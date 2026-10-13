//! `Double.toString` / `Float.toString` 的等价适配器。
//!
//! # 为什么不能直接用 Rust 的 `{}`
//!
//! `CalcJson.value`（`CalcJson.java:236-245`）在非整数分支直接写 `o.toString()`，
//! 也就是 `java.lang.Double.toString`。它和 Rust 的 `format!("{}", d)` 有两处**格式**差异：
//!
//! 1. **记法切换点**：Java 在十进制指数 `<-3` 或 `>=7` 时改用科学计数法
//!    （`Double.toString(1e7)` = `"1.0E7"`、`Double.toString(1e-4)` = `"1.0E-4"`）；
//!    Rust 的 `Display` **永远**用位置记法（`1e7` → `"10000000"`）。
//! 2. **整数尾数带 `.0`**：`Double.toString(100.0)` = `"100.0"`（Java 保证小数点后
//!    至少一位）；Rust 的 `Display` 给 `"100"`。
//!
//! 两边的**有效数字**是一致的（都是"唯一确定该 double 的最短十进制串"，
//! 见 `Double.toString` javadoc "as many, but only as many, more digits as are needed
//! to uniquely distinguish the argument value from adjacent values of type double"），
//! 所以适配器只做**排版**转换：取 Rust 的最短科学记法 `{:e}`，按 Java 的记法规则重排。
//!
//! # 算法（判定式的出处）
//!
//! 令 `exp` = `format!("{:e}", |d|)` 的指数部分，即 `|d| = m × 10^exp`、`1 <= m < 10`
//! （Rust 的 `LowerExp` 恒输出这一规范形）。Java 用位置记法的条件是
//! **`-3 <= exp < 7`** —— 实测：`1e-3`→`"0.001"`（exp = -3，含端点）、
//! `1e-4`→`"1.0E-4"`（exp = -4）、`1e7`→`"1.0E7"`（exp = 7，不含端点）。
//! 位置记法内部再按 `exp` 与有效位数 `n` 的关系决定小数点位置（见 [`from_exp`]）。
//!
//! # 已声明的差异（未核实项，见交付报告）
//!
//! * **最短性只在 javadoc 层面核实**：JDK 19 起 `Double.toString` 才改用 Schubfach
//!   （严格最短）；更老的 `FloatingDecimal` 在**极少数**值上并非最短
//!   （经典例：最小次正规数在旧版输出 `"4.9E-324"`，而最短是 `"5E-324"`）。
//!   本适配器按"严格最短"实现，**没有**逐位对齐过 2^64 个 double。
//! * 本适配器只覆盖**有限非零**数；`NaN`/`±Infinity`/`±0.0` 由构造分支直接给出。
//! * 12 个夹具里 **0 个** 走这条路（实测：`engine_types` 的两个 `double` 是
//!   `92.5` 与 `100`，前者非整数走 `Double.toString(92.5) = "92.5"`，后者被
//!   `CalcJson.java:241-243` 的整数折叠成裸 `100`）；所以本模块的行为**没有夹具钉住**，
//!   只有下面的单测。

/// `java.lang.Double.toString(double)`。
pub fn java_double_to_string(d: f64) -> String {
    if d.is_nan() {
        return "NaN".to_string();
    }
    if d.is_infinite() {
        return if d > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if d == 0.0 {
        // Java 区分正负零：`Double.toString(-0.0)` = `"-0.0"`
        return if d.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    let sci = format!("{:e}", d.abs());
    from_exp(d.is_sign_negative(), &sci)
}

/// `java.lang.Float.toString(float)`：同一套排版规则，但有效数字按 **f32** 的最短串。
pub fn java_float_to_string(f: f32) -> String {
    if f.is_nan() {
        return "NaN".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    let sci = format!("{:e}", f.abs());
    from_exp(f.is_sign_negative(), &sci)
}

/// 把 Rust `{:e}` 的最短科学记法重排成 Java 的写法。`sci` 形如 `"1e7"` / `"3.0000000000000004e-1"`
/// （不带符号；符号由 `neg` 给出）。
fn from_exp(neg: bool, sci: &str) -> String {
    let (mant, exp) = sci
        .split_once('e')
        .unwrap_or_else(|| panic!("{{:e}} 的输出必含 'e'，实得 {sci:?}"));
    let sci_exp: i32 = exp.parse().unwrap_or_else(|_| panic!("指数应是整数，实得 {exp:?}"));
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let n = digits.len() as i32;

    let mut out = String::new();
    if neg {
        out.push('-');
    }
    // Java 的记法切换：`-3 <= exp < 7` 走位置记法，否则科学计数法
    if (-3..7).contains(&sci_exp) {
        if sci_exp >= n - 1 {
            // 纯整数：补零到小数点前，再补 Java 要求的 ".0"
            out.push_str(&digits);
            for _ in 0..(sci_exp - (n - 1)) {
                out.push('0');
            }
            out.push_str(".0");
        } else if sci_exp >= 0 {
            let k = (sci_exp + 1) as usize;
            out.push_str(&digits[..k]);
            out.push('.');
            out.push_str(&digits[k..]);
        } else {
            out.push_str("0.");
            for _ in 0..(-sci_exp - 1) {
                out.push('0');
            }
            out.push_str(&digits);
        }
    } else {
        // 科学记数法：Java 写大写 E、无 "+"、无前导零；尾数保证有小数部分
        out.push_str(mant);
        if !mant.contains('.') {
            out.push_str(".0");
        }
        out.push('E');
        out.push_str(exp);
    }
    out
}
