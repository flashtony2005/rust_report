//! 参数表：`calculate_with` 的输入之一（**M3-01 参数通道**）。
//!
//! # 与产品的对应
//!
//! 产品把参数值放在 `Context.params`（`Map<String, Object>`，`Context.java:30`），
//! 建表达式时**当场取值内联**成 `Constant(值)`（`Expression.java:185-188`）：
//!
//! | 源码写法 | 登记与否 | 产品行为（源码位置） |
//! |---------|---------|--------------------|
//! | `@p`   | 已登记 | `Constant(getParamValue(p))`（`:187-188`） |
//! | `@p`   | 未登记 | `ReportError(unknownParam)`（`:189-191`，真机消息见 RUNLOG §24.1） |
//! | `@@p`  | 无论 | `Constant(getParamValue(p))`（`:183-186`，**不查** `isParamName`；未登记 ⇒ `getParamValue` 返回 null ⇒ `Constant(null)`） |
//! | 裸名    | 已登记 | `ParamNode(name, getParamValue(name))`（`:233`；`ParamNode extends Constant`，行为与 `Constant(值)` 一致） |
//!
//! ⇒ 本 crate 的实现方式是**建树期绑定**：参数值在 [`crate::expr::parse_with`] 期
//! 就内联进 AST（与产品同为 `Constant`），**不做求值期查询**。
//!
//! # 为什么不做求值期兜底（一个被证伪的早期设计）
//!
//! 早期设计曾留 `Expr::Param { value: Option<Value> }` + 求值期 `ctx.param()`
//! 兜底，理由是"拿不到参数值时保守取 false，求值期再用真实值重判"。但
//! `is_extend` 是**准备期**判定（`ExtNormalCell.checkExtend`，驱动扩展格的拓扑
//! 改写）——求值期才拿到值**救不了扩展**：扩展已经错在同一条链上。留着兜底只会
//! 掩盖"建树期没绑定"的 bug（"因为错的原因得出对的答案"）。M3-01 落地时删除该
//! 路径，参数语义完全由本表 + 建树期绑定承担（`spec-m3.md` §3.3）。
//!
//! # 空表与"未登记"的关系
//!
//! [`ParamTable::EMPTY`] 是**既有调用点**（`calculate` 无参版、内部合成表达式）
//! 的输入：它等价于"没有任何参数登记"，因此模板里写 `@p` 会在解析期
//! `Err(UnknownParam)` —— **显式失败**，不是静默当无参数算（15 个既有夹具都不含
//! 参数，不受影响）。

use std::collections::BTreeMap;

use crate::value::Value;

/// 报表参数值表（产品的 `Context.params`）。
///
/// `is_param` 与 `get` 的区别是**契约**不是风格：产品 `isParamName` 查的是
/// `containsKey`（`Context.java:133-135`），而 `getParamValue` 可以返回 null
/// （`:143-151`）⇒ "登记但值为 null"（如 Servlet 层空串 → null，`ReportUtils2.java:236-238`）
/// 与"未登记"是两种不同状态：前者 `@p` 不报错、值为 null；后者报 `UnknownParam`。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParamTable {
    entries: BTreeMap<String, Value>,
}

impl ParamTable {
    /// 空表常量 —— 无参数场景的输入（`calculate` 的默认入口）。
    pub const EMPTY: ParamTable = ParamTable {
        entries: BTreeMap::new(),
    };

    /// 空表（同 [`ParamTable::EMPTY`]，惯用构造器形式）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一个参数值（产品的 `setParamValue`，`Context.java:163-168`。
    /// 同名重复登记**覆盖**——与 `params.put` 一致）。
    pub fn insert(&mut self, name: impl Into<String>, value: Value) {
        self.entries.insert(name.into(), value);
    }

    /// `Context.isParamName`（`:133-135`）：名字**是否登记**（值可以是 null）。
    ///
    /// `@p` 的建树（`Expression.java:187`）查这一条。
    pub fn is_param(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// `Context.getParamValue`（`:143-151`）：取登记的值；未登记返回 `None`
    /// （产品返回 null）。
    ///
    /// `@@p` 的建树（`Expression.java:185`）用这一条，且**不**先用 `is_param` 过滤。
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.entries.get(name)
    }

    /// 登记个数（调试/夹具断言用）。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 空表判定。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl FromIterator<(String, Value)> for ParamTable {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        ParamTable {
            entries: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_table_has_no_params() {
        assert!(!ParamTable::EMPTY.is_param("p"));
        assert_eq!(ParamTable::EMPTY.get("p"), None);
        assert!(ParamTable::EMPTY.is_empty());
    }

    #[test]
    fn a_registered_null_is_still_registered() {
        // "登记但值为 null" 与 "未登记" 是两种状态（模块文档：Servlet 空串 → null）。
        let mut t = ParamTable::new();
        t.insert("p", Value::Null);
        assert!(t.is_param("p"), "containsKey 语义：登记即真");
        assert_eq!(t.get("p"), Some(&Value::Null));
    }

    #[test]
    fn insert_overwrites_like_a_java_hash_map() {
        let mut t = ParamTable::new();
        t.insert("p", Value::Int(1));
        t.insert("p", Value::Int(2));
        assert_eq!(t.get("p"), Some(&Value::Int(2)));
        assert_eq!(t.len(), 1);
    }
}
