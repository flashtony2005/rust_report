//! 宏声明（定义 JSON 的顶层 `macros` 块 → [`rpx_engine::MacroTable`]）。
//!
//! # 形态：**对象**，不是数组
//!
//! ```json
//! "macros": { "quarter": "Q1", "qcol": "ds1.qty" }
//! ```
//!
//! ⛔ 与 `params[]`（数组 of 对象）**刻意不同**：参数有 5 个槽（类型/默认值/说明…），
//! 非数组装不下；宏**只有名字和值两样**，而且值就是"没有数据类型的字符串"
//! （`报表引擎设计.md:244`）⇒ 对象才是它的自然形状，`[{"name":…,"value":…}]`
//! 是纯冗余。两者的差异是**形状跟着语义走**，不是不一致。
//!
//! # 值必须是字符串（不猜、不静默转）
//!
//! `{"n": 3}` **显式拒绝**，不偷偷 `to_string`。理由与 `params[].default` 同款
//! （`params.rs` 的注释）：宏是**文本替换**，`3` 与 `"3"` 落进表达式是同一串、
//! 但 `3.0` / `true` / `null` 的文本形态有歧义 —— 让写的人自己决定。
//!
//! # RPX 持久化
//!
//! 宏写入 `rust_report.macro.<名称>` 自定义属性。保存、读取与直接计算共享同一张表。
//! 产品私有 MacroMetaData 只保留原始字节，不猜测它的内部结构。
//!
//! 契约正文见 `docs/report-def-json.md` §3.7 / §4.9。

use serde_json::{Map, Value as J};

use rpx_engine::MacroTable;

use super::json::{err, kind_of, obj};
use super::DefError;

/// 顶层 `macros` 块 → 宏表（缺省 ⇒ 空表，不是错误）。
pub(super) fn parse_macros(root: &Map<String, J>) -> Result<MacroTable, DefError> {
    let Some(v) = root.get("macros") else {
        return Ok(MacroTable::new());
    };
    let m = obj(v, "$.macros")?;
    let mut out = MacroTable::new();
    for (name, value) in m {
        let path = format!("$.macros.{name}");
        if name.is_empty() {
            return Err(err(&path, "宏名不能为空"));
        }
        // ⛔ 值必须是字符串：`{"n":3}` 不偷偷转（见模块文档）。
        let J::String(s) = value else {
            return Err(err(&path, format!("应为字符串，实得 {}", kind_of(value))));
        };
        out.insert(name.clone(), s.clone());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(root: &str) -> Result<MacroTable, DefError> {
        let r: Map<String, J> = serde_json::from_str(root).expect("测试 JSON 合法");
        parse_macros(&r)
    }

    /// 缺省 `macros` ⇒ 空表（不是错误；绝大多数定义没有宏）。
    #[test]
    fn absent_macros_is_an_empty_table() {
        assert!(parse(r#"{"grid":{"rows":1,"cols":1}}"#).unwrap().is_empty());
        assert!(parse(r#"{"macros":{}}"#).unwrap().is_empty());
    }

    /// 名字与值都原样落地（不做任何转义/裁剪）。
    #[test]
    fn names_and_values_land_verbatim() {
        let t = parse(r#"{"macros":{"quarter":"Q1","qcol":"ds1.qty"}}"#).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t.get("quarter"), Some("Q1"));
        assert_eq!(t.get("qcol"), Some("ds1.qty"));
    }

    /// 值里可以再引用宏（替换是**迭代到不动点**的，见 `rpx_engine::macros`）。
    #[test]
    fn a_value_may_reference_another_macro() {
        let t = parse(r#"{"macros":{"a":"${b}+1","b":"B2*2"}}"#).unwrap();
        assert_eq!(t.get("a"), Some("${b}+1"), "定义期不做替换，替换在解析期");
        assert_eq!(rpx_engine::substitute_macros("${a}", &t).unwrap(), "B2*2+1");
    }

    /// ⛔ 值必须是字符串：数字/布尔/null/数组/对象一律**显式拒绝**（不偷偷 `to_string`）。
    #[test]
    fn a_non_string_value_is_rejected_loudly() {
        for (root, want) in [
            (r#"{"macros":{"n":3}}"#, "数字"),
            (r#"{"macros":{"b":true}}"#, "布尔"),
            (r#"{"macros":{"z":null}}"#, "null"),
            (r#"{"macros":{"a":[1]}}"#, "数组"),
            (r#"{"macros":{"o":{}}}"#, "对象"),
        ] {
            let e = parse(root).unwrap_err();
            let msg = format!("{e}");
            assert!(msg.starts_with("$.macros."), "路径应指向宏名，实得 {msg}");
            assert!(msg.contains(want), "应说明实得类型 {want}：{msg}");
        }
    }

    /// `macros` 本身不是对象 ⇒ 带路径报错。
    #[test]
    fn a_malformed_macros_block_carries_the_path() {
        let e = parse(r#"{"macros":[{"name":"a","value":"1"}]}"#).unwrap_err();
        let msg = format!("{e}");
        assert!(msg.starts_with("$.macros"), "实得 {msg}");
        assert!(msg.contains("应为对象"), "实得 {msg}");
    }
}
