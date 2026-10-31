//! `java.math.BigDecimal.toString()` 的复刻 —— 字面量取值里**唯一需要"算"的一支**。
//!
//! # 为什么必须自己算
//!
//! `rpx_core` 按 M1 契约**只透传** `BigDecimal` 的 `(scale, unscaled)` 原始形态
//! （`docs/format-contract.md` 附件 A：标签 2 = int32 scale + 大端补码 unscaled 字节），
//! 不转十进制；而夹具的 `value` 段写的是 `o.toString()`（`CalcJson.java:233-235`）。
//! 本 crate 零第三方依赖（不引 bigdecimal 库），丢了精度换成 f64 又恰好是契约禁止的事
//! —— 所以只能把 Java 的 `toString` 规则逐条搬过来。
//!
//! 夹具钉子：`engine_types` 的 `(4,1)` = `new BigDecimal("100.00")`
//! → 计算的 JSON 原文是 `"value":100.00`（**不是** `100.0`：那两个尾零是精度信息，
//! 丢了 `BigDecimal("100.00")` 与 `BigDecimal("100")` 就分不开了）。

/// `java.math.BigDecimal.toString()` 的复刻。
///
/// 契约（`java.math.BigDecimal` JavaDoc "toString" + OpenJDK `layoutChars`）：
/// 设 `unscaled` 的十进制位数为 `precision`、`adjusted = -scale + precision - 1`，则
///
/// | 条件 | 形态 |
/// |------|------|
/// | `scale == 0` | `sign + digits` |
/// | `scale > 0 && adjusted >= -6` | 普通形态：插小数点；整数位不足则补 `0`（`0.00…digits`） |
/// | **其余（含 `scale < 0`）** | **指数形态**：`d[0][.d[1..]]E{+|-}\|adjusted\|` |
///
/// ⛔ **`scale < 0` 走指数形态，不是"补零写全"** —— 这一条与直觉相反，本轮实测踩实：
/// 产品自带 JRE 1.8.0_151 与 JDK 21 都给出 `(-3, 1) → "1E+3"`、`(0, -3) → "0E+3"`、
/// `(-10, 12345) → "1.2345E+14"`。之前的实现按"补零"写，`(-3, 1)` 给出 `1000`、
/// `(-3, 0)` 甚至给出 `0000`。golden 出处见 `tests/fixtures/ProbeBigDecimalToString.java`
/// 与下方单测的注释（复现命令：用 JDK 编译成 Java 8 字节码后，分别用产品自带 JRE
/// `common/jre/bin/java.exe` 与 JDK 21 跑，两者输出逐字节一致）。
///
/// 其它实测细节：指数**恒带符号**（`E+3` / `E-10`）；尾零**保留**（`(-1, 12300) → "1.2300E+5"`、
/// `(-2, 1000) → "1.000E+5"`）；零带精度时 `(10, 0) → "0E-10"`。
pub fn decimal_to_string(scale: i32, unscaled: &[u8]) -> String {
    let (negative, magnitude) = twos_complement_magnitude(unscaled);
    let digits = bytes_to_decimal(&magnitude); // 大端十进制数字串，无前导零（0 → "0"）
    let sign = if negative { "-" } else { "" };
    let precision = digits.len() as i64;
    let adjusted = -(scale as i64) + precision - 1;

    if scale == 0 {
        return format!("{sign}{digits}");
    }
    // 普通形态：`scale > 0 && adjusted >= -6`
    if scale > 0 && adjusted >= -6 {
        let scale = scale as usize;
        if precision as usize > scale {
            let (int_part, frac) = digits.split_at(digits.len() - scale);
            return format!("{sign}{int_part}.{frac}");
        }
        return format!("{sign}0.{}{digits}", "0".repeat(scale - digits.len()));
    }
    // 指数形态（`scale < 0`，或 `scale > 0` 但 `adjusted < -6`）：
    // 单数字整数位 + 可选小数位 + `E` + 带符号指数。
    // 两个来源的指数符号天然确定：`scale < 0` ⇒ `adjusted >= precision >= 1 > 0`；
    // `adjusted < -6` ⇒ 必为负 —— 但仍按 `adjusted` 的符号取，不靠前提硬编码。
    let mantissa = if precision > 1 {
        format!("{}.{}", &digits[..1], &digits[1..])
    } else {
        digits.clone()
    };
    let exp_sign = if adjusted < 0 { '-' } else { '+' };
    format!("{sign}{mantissa}E{exp_sign}{}", adjusted.abs())
}

/// 大端补码字节 → `(是否负数, 绝对值字节)`。
///
/// 负数的绝对值 = 取反加一（Java `BigInteger.toByteArray()` 的逆运算）。
/// 全零输入（`unscaled == 0`）恒为正 —— `BigDecimal` 没有 -0。
fn twos_complement_magnitude(bytes: &[u8]) -> (bool, Vec<u8>) {
    if bytes.is_empty() {
        return (false, vec![0]);
    }
    if bytes[0] & 0x80 == 0 {
        return (false, bytes.to_vec());
    }
    let mut mag: Vec<u8> = bytes.iter().map(|b| !*b).collect();
    let mut carry = 1u16;
    for b in mag.iter_mut().rev() {
        let v = *b as u16 + carry;
        *b = (v & 0xFF) as u8;
        carry = v >> 8;
    }
    (true, mag)
}

/// 大端字节串 → 十进制数字串（无前导零；全零 → `"0"`）。
///
/// 逐字节 `十进制累加器 ×256 + 字节`，十进制累加器用**小端数字**数组表示
/// （不引入 bignum 依赖：`unscaled` 位数由模板决定，实际都远小于数组规模上限）。
fn bytes_to_decimal(bytes: &[u8]) -> String {
    let mut digits: Vec<u8> = vec![0]; // 小端：digits[0] 是个位
    for b in bytes {
        let mut carry = *b as u32;
        for d in digits.iter_mut() {
            let v = (*d as u32) * 256 + carry;
            *d = (v % 10) as u8;
            carry = v / 10;
        }
        while carry > 0 {
            digits.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    while digits.len() > 1 && *digits.last().unwrap() == 0 {
        digits.pop(); // 去掉高位零
    }
    digits.iter().rev().map(|d| (b'0' + d) as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ⚠ 下面所有期望值都是**实跑**取来的，不是按文档推的。
    // 复现：
    //   cd tests/fixtures/_bdo && javac -d out -source 8 -target 8 ProbeBigDecimalToString.java
    //   "F:/project/report/common/jre/bin/java.exe" -cp out rpx.fixtures.ProbeBigDecimalToString
    //   /d/Tools/jdk-21.0.2/bin/java.exe            -cp out rpx.fixtures.ProbeBigDecimalToString
    // 两个 JVM（产品自带 1.8.0_151 / JDK 21.0.2）输出逐字节一致。

    #[test]
    fn decimal_matches_java_to_string_on_the_fixture_pin() {
        // engine_types (4,1) = new BigDecimal("100.00") → JSON 原文 `100.00`
        // unscaled = 10000, scale = 2 → 大端补码 [0x27,0x10]
        assert_eq!(decimal_to_string(2, &[0x27, 0x10]), "100.00");
    }

    #[test]
    fn decimal_plain_form_covers_scale_zero_and_positive() {
        assert_eq!(decimal_to_string(0, &[100]), "100"); // scale 0：整数字面量
        assert_eq!(decimal_to_string(0, &[0]), "0");
        assert_eq!(decimal_to_string(1, &[5]), "0.5"); // 整数位补零
        assert_eq!(decimal_to_string(3, &[1]), "0.001");
        assert_eq!(decimal_to_string(1, &[100]), "10.0");
        assert_eq!(decimal_to_string(4, &[0x30, 0x39]), "1.2345");
        assert_eq!(decimal_to_string(2, &[0]), "0.00"); // BigDecimal 零带精度
                                                        // `adjusted == -6` 是**普通形态**的边界（Java 判据是 `>= -6`）
        assert_eq!(decimal_to_string(6, &[1]), "0.000001");
        // `adjusted == -4` 仍走普通形态
        assert_eq!(decimal_to_string(8, &[0x30, 0x39]), "0.00012345");
    }

    #[test]
    fn decimal_uses_exponential_notation_for_small_adjusted_exponents() {
        // adjusted = -7 / -10 / -30 < -6 ⇒ 指数形态，指数**带负号**
        assert_eq!(decimal_to_string(7, &[1]), "1E-7");
        assert_eq!(decimal_to_string(10, &[1]), "1E-10");
        assert_eq!(decimal_to_string(30, &[1]), "1E-30");
        // 零带精度且 adjusted < -6 也是指数形态（不是 "0.0000000000"）
        assert_eq!(decimal_to_string(10, &[0]), "0E-10");
    }

    #[test]
    fn decimal_negative_scale_goes_exponential_not_zero_padded() {
        // ⛔ 与直觉相反的一组：`scale < 0` 一律指数形态，且指数**带正号**。
        // 这条是本轮的 blocking 修复点 —— 旧实现把 (-3,1) 写成 "1000"、(-3,0) 写成 "0000"。
        assert_eq!(decimal_to_string(-3, &[1]), "1E+3");
        assert_eq!(decimal_to_string(-1, &[5]), "5E+1");
        assert_eq!(decimal_to_string(-5, &[1]), "1E+5");
        assert_eq!(decimal_to_string(-3, &[0]), "0E+3");
        assert_eq!(decimal_to_string(-10, &[1]), "1E+10");
        assert_eq!(decimal_to_string(-10, &[0x30, 0x39]), "1.2345E+14");
        // 尾零**保留**（不是 "1.23E+5"）
        assert_eq!(decimal_to_string(-1, &[0x30, 0x0C]), "1.2300E+5");
        assert_eq!(decimal_to_string(-2, &[0x03, 0xE8]), "1.000E+5");
        // 负值 + 负 scale
        assert_eq!(decimal_to_string(-1, &[0xCF, 0xF4]), "-1.2300E+5");
    }

    #[test]
    fn decimal_handles_the_sign_bit() {
        // -100.00 → unscaled = -10000，补码 [0xD8,0xF0]
        assert_eq!(decimal_to_string(2, &[0xD8, 0xF0]), "-100.00");
        assert_eq!(decimal_to_string(0, &[0xFF]), "-1");
        assert_eq!(decimal_to_string(0, &[0x9C]), "-100");
        assert_eq!(decimal_to_string(0, &[0xFB]), "-5");
    }

    #[test]
    fn bytes_to_decimal_reads_big_endian_multi_byte_magnitudes() {
        assert_eq!(bytes_to_decimal(&[1, 0, 0, 0]), "16777216"); // 256³
        assert_eq!(
            bytes_to_decimal(&[0x01, 0x1F, 0x71, 0xFB, 0x04, 0xCB]),
            "1234567890123"
        );
        assert_eq!(
            decimal_to_string(0, &[0x01, 0x1F, 0x71, 0xFB, 0x04, 0xCB]),
            "1234567890123"
        );
    }
}
