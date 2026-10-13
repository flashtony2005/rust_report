//! 测试专用**手写** JSON 解析器（本 crate 零第三方依赖，用不了 serde_json）。
//!
//! 它只服务于一件窄事：把产品产出的 `engine_*.expected.json` 忠诚地读进来。
//! 三条设计要点，每条都对应一个真实坑：
//!
//! 1. **原文保真（最关键）**：每个节点保存它在输入里的**原始切片** [`Json::raw`]。
//!    产品 writer `CalcJson.java:204-212` 会把 `Double` 的整数值打成裸整数
//!    （`if (d == Math.floor(d) && Math.abs(d) < 1e15) return Long.toString((long) d);`），
//!    并把类型名单独输出（`CalcJson.java:224-253` 的 `typeName`）。于是
//!    `{"value":150,"type":"double"}` 是合法的。若在解析期把它 `parse::<f64>()`
//!    再回写，`150` 会变 `150.0`、`100.00`（`BigDecimal.toString()`，
//!    `CalcJson.java:201-203`）会变 `100.0` —— M2-14 类型保真就无从断言。
//!    故数字**只记原文，不做任何数值归一化**。
//! 2. **转义解码**：错误型夹具的 `message` 含 `\uXXXX`（见
//!    `engine_cycle.expected.json`）；必须真正解成中文，否则断言只能比对转义串。
//! 3. **响亮失败**：形状不符一律返回 `Err`，由调用方 panic。**绝不静默给默认值** ——
//!    静默通过的测试比失败的测试更坏。
//!
//! 拆成三片（受"单文件 ≤300 行"门禁约束）：本文件放数据结构与入口，
//! [`parser`] 放递归下降解析，[`access`] 放取值器。

mod access;
mod parser;

pub use parser::parse;

/// 解析出的 JSON 节点。`raw` 是该节点在输入里的原文切片（见模块文档第 1 条）。
#[derive(Debug, Clone)]
pub struct Json {
    pub raw: String,
    pub kind: Kind,
}

/// JSON 值的六种形态。`Num` 不带数值，数值只存在于 [`Json::raw`]。
#[derive(Debug, Clone)]
pub enum Kind {
    Null,
    Bool(bool),
    /// 数字：**原文在 `raw`**（`10.00` 与 `10` 必须可分）。
    Num,
    /// 字符串：已解码（`\uXXXX` / `\n` / `\"` / `\\` …）。
    Str(String),
    Arr(Vec<Json>),
    /// 对象：保持输入顺序（便于 `expect_keys` 报出第一个未识别键）。
    Obj(Vec<(String, Json)>),
}
