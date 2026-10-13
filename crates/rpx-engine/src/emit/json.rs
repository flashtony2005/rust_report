//! 零依赖 JSON 文本构造 —— `tests/fixtures/Json.java` 的等价物。
//!
//! 为什么不引 serde：本 crate 的 `Cargo.toml` 只有 `rpx-core` 一个依赖（M2 硬门禁
//! 「零第三方依赖」）；而夹具生成器 `Json.java` 本身就是手写的几十行构造器 ——
//! 复刻它比引一个库更贴近地面真值，也避免序列化器的默认行为（键排序、转义策略）
//! 与产品不一致。
//!
//! # 三条必须逐字对齐的规则
//!
//! 1. **键序 = 插入序**。`Json.java:12/14/19-24` 用 `StringBuilder` 顺序追加，没有
//!    排序；所以本模块的 [`obj`] 按入参 `fields` 的先后写，**不许**改成 BTreeMap。
//! 2. **紧凑单行**：无空格、无换行、无缩进（`Json.java` 全程 `append`）。这不是
//!    "省字节"，而是让引擎输出与夹具原文能做**逐字符 diff**。
//! 3. **转义**（`Json.java:104-143`）：`"` `\` `\b` `\f` `\n` `\r` `\t` 用短转义，
//!    **其余 `c < 0x20 || c > 0x7E` 一律写 `\uXXXX`（小写十六进制、固定 4 位）**。
//!    判据里的 `> 0x7E` 意味着**非 ASCII 也必须转义**（不是"可转可不转"）：
//!    `engine_disp.expected.json` 的 `disp_value` 原文就是 `"\u4ec5\u663e\u793a\u503c"`
//!    （= `仅显示值`），实测坐实。
//!
//! # 为什么按 UTF-16 码元迭代
//!
//! Java 的 `char` 是 16 位，`Json.java:110-140` 逐 `char` 处理；一个 BMP 外的字符
//! （如 emoji）在 Java 里是**两个** `char`（代理对），会写成两个 `\uXXXX`。
//! Rust 的 `char` 是 32 位，若按 `char` 迭代就会把它当成 `> 0x7E` 的一个字符 ——
//! 数值对不上。故这里用 [`str::encode_utf16`] 逐码元迭代，与 Java 逐 `char` 等价。

/// `Json.quote(String)`（`Json.java:104-143`）：加引号并转义。`None` 语义由调用方
/// 用 [`crate::emit::render_value`] 处理（那里写裸 `null`）。
pub fn quote(s: &str) -> String {
    let mut b = String::with_capacity(s.len() + 2);
    b.push('"');
    for u in s.encode_utf16() {
        match u {
            0x22 => b.push_str("\\\""),
            0x5C => b.push_str("\\\\"),
            0x08 => b.push_str("\\b"),
            0x0C => b.push_str("\\f"),
            0x0A => b.push_str("\\n"),
            0x0D => b.push_str("\\r"),
            0x09 => b.push_str("\\t"),
            _ if !(0x20..=0x7E).contains(&u) => b.push_str(&format!("\\u{:04x}", u)),
            // 只有 0x20..=0x7E 会走到这里，故 `u as u8 as char` 恒等且不会引入多字节
            _ => b.push(u as u8 as char),
        }
    }
    b.push('"');
    b
}

/// `JsonObj.put(...)` 的整体产物：`{"k":v,...}`，**顺序即 `fields` 顺序**。
pub fn obj(fields: Vec<(&'static str, String)>) -> String {
    let mut b = String::from("{");
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            b.push(',');
        }
        b.push_str(&quote(k));
        b.push(':');
        b.push_str(v);
    }
    b.push('}');
    b
}

/// `JsonArr.add(...)` 的整体产物：`[v,...]`。
pub fn arr(items: Vec<String>) -> String {
    let mut b = String::from("[");
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            b.push(',');
        }
        b.push_str(it);
    }
    b.push(']');
    b
}

/// 裸整数（`JsonObj.put(String,long)` → `Long.toString`）。
pub fn int(v: i64) -> String {
    v.to_string()
}

/// 裸 8 位整数（`source.extend` 用的就是这种裸数字，见 `CalcJson.java:99`）。
pub fn int8(v: i8) -> String {
    v.to_string()
}
