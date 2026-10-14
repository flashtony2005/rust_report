//! `BigDecimal.toString` 复刻的**产品实跑回归**（被测对象：`src/literal/decimal.rs`）。
//!
//! # 为什么必须单独有这条判据
//!
//! `engine_types` 夹具里 `type=decimal` 只有**一个**值（`(4,1)` = `100.00`），而
//! `BigDecimal.toString` 有四种形态：`scale==0` / 普通 / 指数（`adjusted < -6`）/
//! **`scale<0` 的指数形态**。只比夹具 ⇒ 后面三种规则**没有任何测试管**
//! —— 本轮独立复核正是在这里抓到一个"方向相反"的实现（把 `scale<0` 写成补零）。
//!
//! 所以这条判据直接吃**真 JVM 实跑出来的 oracle**（禁止手写、禁止同源复制）：
//! `tests/fixtures/ProbeBigDecimalToString.java` 分别用 **JDK 8** 与 **JDK 21** 跑一遍，
//! 输出落成 `tests/fixtures/_bdo/out_jre8.txt` / `out_jdk21.txt`。两个 JVM
//! **除版本头行外逐字节相同**这一点本身就是一条断言 —— 它把"该规则是 JDK 规范、
//! 不是某版实现细节"钉住，也让"基准到底该用哪个 JRE 跑"永远不必再问。
//!
//! # oracle 的实际出处（2026-10-13，如实记录）
//!
//! | 文件 | 生成者 | 版本头 |
//! |---|---|---|
//! | `out_jre8.txt`  | Azul Zulu OpenJDK **8.0.504**（macOS aarch64） | `java.version=1.8.0_504` |
//! | `out_jdk21.txt` | Azul Zulu OpenJDK **21.0.12.1**（macOS aarch64） | `java.version=21.0.12.1` |
//!
//! ⚠ **不是**产品自带的那个 JRE：仓库自带的 `common/jre` 是 **Windows** 版
//! （`bin/java.exe` + 一堆 `.dll`），在 macOS 上跑不了，所以无法在那个 JRE 上重跑。
//! 这不影响判据有效性 —— `BigDecimal.toString()` 是 JDK 规范行为、纯 Java 实现，
//! 且**下方 `the_two_jvm_oracles_agree_byte_for_byte` 恰好就是用来证明
//! "换个 JVM 结果一样"的那条断言**；真要在产品 JRE 上复跑，见脚本参数。
//!
//! 复跑命令（`scripts/gen_decimal_oracle.sh`，会自检并在失败时清掉产物）：
//!
//! ```text
//! scripts/gen_decimal_oracle.sh \
//!   --javac <JDK21>/bin/javac \
//!   --jre8  <JDK8>/bin/java \
//!   --jdk21 <JDK21>/bin/java
//! ```
//!
//! # oracle 格式（每行 5 段，`|` 分隔）
//!
//! ```text
//! scale | unscaled 补码字节(hex,逗号分隔) | unscaled 十进制 | toString() | toPlainString()
//! ```
//!
//! 例：`-3|01|1|1E+3|1000` —— `new BigDecimal(BigInteger.valueOf(1), -3)` 的
//! `toString()` 是 **`1E+3`**（指数形态），而 `toPlainString()` 才是 `1000`。
//! 第 5 段（plain）**故意不参与断言**：`rpx` 复刻的是 `toString()`（夹具写的是
//! `o.toString()`，`CalcJson.java:233-235`），把 plain 也拉进来会掩盖形态判据。

/// oracle 的一行（只取对拍需要的 4 段）。
struct Case {
    /// 与 `decimal_to_string` 的第一个参数同义。
    scale: i32,
    /// 大端补码字节（`BigInteger.toByteArray()` 的形态）。
    bytes: Vec<u8>,
    /// `unscaled` 的十进制值（`scale == 0` 时就是 `toString()` 的结果）。
    unscaled_decimal: String,
    /// 产品 JRE 的 `toString()` —— 本文件的**唯一**期望值来源。
    to_string: String,
}

fn oracle_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/_bdo")
        .join(name)
}

fn read_oracle(name: &str) -> Vec<Case> {
    let text = std::fs::read_to_string(oracle_path(name))
        .unwrap_or_else(|e| panic!("读取 {name} 失败：{e}"));
    parse_oracle(&text)
}

/// 解析 oracle 正文：跳过 `java.version=` 头行（`lines()` 会吃掉 CRLF 的 `\r`）。
fn parse_oracle(text: &str) -> Vec<Case> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with("java.version=") {
            continue;
        }
        let f: Vec<&str> = line.split('|').collect();
        assert_eq!(f.len(), 5, "oracle 行格式变了（应 5 段）：{line:?}");
        out.push(Case {
            scale: f[0]
                .parse()
                .unwrap_or_else(|_| panic!("scale 非法：{line:?}")),
            bytes: parse_hex_bytes(f[1], line),
            unscaled_decimal: f[2].to_string(),
            to_string: f[3].to_string(),
        });
    }
    out
}

fn parse_hex_bytes(field: &str, line: &str) -> Vec<u8> {
    if field.is_empty() {
        return Vec::new();
    }
    field
        .split(',')
        .map(|h| u8::from_str_radix(h, 16).unwrap_or_else(|_| panic!("字节非法（{h}）：{line:?}")))
        .collect()
}

/// 去掉版本头行后的正文（用于跨 JVM 比对）。
fn body_without_version(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| !l.starts_with("java.version="))
        .collect()
}

/// 两个 JVM 的输出必须**逐字节一致**（只有版本头行允许不同）。
///
/// 这条断言把"基准是 JDK 规范"钉住：若哪天 `toString` 变成实现细节，
/// 这里会先红 —— 那时才知道"该按哪个 JRE 实现"是需要作决策的事。
#[test]
fn the_two_jvm_oracles_agree_byte_for_byte() {
    let jre8 = std::fs::read_to_string(oracle_path("out_jre8.txt")).unwrap();
    let jdk21 = std::fs::read_to_string(oracle_path("out_jdk21.txt")).unwrap();
    assert_eq!(
        body_without_version(&jre8),
        body_without_version(&jdk21),
        "两个 JVM 的 toString 输出不一致 —— 该规则不再是 JDK 规范，需重新作决策"
    );
    assert_ne!(
        jre8.lines().next(),
        jdk21.lines().next(),
        "两份 oracle 的版本头行相同 —— 可能实际测的是同一份，跨 JVM 比对已失去意义"
    );
}

/// 主判据：逐行断言 `decimal_to_string` 与产品 JRE 的 `toString()` 完全一致。
///
/// 覆盖度自检（`assert` 那几条）是刻意的：若 oracle 只剩普通形态，
/// "全绿"就毫无意义，所以先证明四种形态都有代表再看结论。
#[test]
fn decimal_to_string_matches_the_product_jre_on_every_probed_case() {
    let cases = read_oracle("out_jre8.txt");
    assert!(
        cases.len() >= 25,
        "oracle 用例过少（{}）—— 文件被截断？",
        cases.len()
    );

    assert!(cases.iter().any(|c| c.scale == 0), "缺 scale==0 的用例");
    assert!(
        cases
            .iter()
            .any(|c| c.scale > 0 && !c.to_string.contains('E')),
        "缺普通形态（scale>0 且 adjusted>=-6）的用例"
    );
    assert!(
        cases
            .iter()
            .any(|c| c.scale > 0 && c.to_string.contains('E')),
        "缺 scale>0 但走指数形态（adjusted<-6）的用例"
    );
    assert!(
        cases
            .iter()
            .any(|c| c.scale < 0 && c.to_string.contains('E')),
        "缺 scale<0 的指数形态用例 —— 这正是本轮被写反的那一支"
    );

    for c in &cases {
        assert_eq!(
            rpx_engine::decimal_to_string(c.scale, &c.bytes),
            c.to_string,
            "scale={} bytes={:02X?} 与产品 JRE 不符",
            c.scale,
            c.bytes
        );
    }
}

/// 副判据：`scale == 0` 分支早返回 `sign + digits` ⇒ 它同时是**补码解码**的独立判据
/// （oracle 第 3 段就是 `unscaled` 的十进制值，即 `bytes_to_decimal` 的期望输出）。
#[test]
fn scale_zero_also_pins_the_twos_complement_decoding() {
    let zero: Vec<Case> = read_oracle("out_jre8.txt")
        .into_iter()
        .filter(|c| c.scale == 0)
        .collect();
    assert!(
        zero.len() >= 5,
        "scale==0 的用例过少（{}）—— 补码解码没被覆盖",
        zero.len()
    );
    for c in &zero {
        assert_eq!(
            rpx_engine::decimal_to_string(0, &c.bytes),
            c.unscaled_decimal,
            "bytes={:02X?} 的补码解码与产品不符",
            c.bytes
        );
    }
}
