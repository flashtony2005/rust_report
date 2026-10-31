//! 宏（`${名}` / `%{名}`）—— **解析表达式之前**的纯字符串替换。
//!
//! # 事实源
//!
//! | 结论 | 出处 |
//! |------|------|
//! | 宏是「**没有数据类型的字符串**」，替换的是**宏变量所占的位置** | `报表引擎设计.md` §二 #7（`:244`）与 §五 表（`:955`） |
//! | 可放在**任何位置**：单元格值、表达式、数据集定义、单元格属性表达式 | 同上（原文列举） |
//! | 替换发生在**建表达式之前** | `docs/m2-engine-design.md:1585`：`MacroResolver2.replaceMacros(expr, ctx)` 在 `new Expression(...)` **之前**一行 |
//! | SQL 数据集侧的记号是 `${...}` | `tests/fixtures/RUNLOG.md` §34.1（`SQLDataSetFactory.createDataSetM`：宏 `${...}` 替换 → `?` 占位参数求值） |
//! | 设计文档的记号是 `%{名}` | `报表引擎设计.md:244` 原文「引用方法为 `%{宏变量名}`」 |
//!
//! ⇒ **两个记号都收**。用户的需求原文写的也是 `%{}`，而产品实证是 `${}`；
//! 二者语义相同（都是"用宏值替换所占位置"），所以本模块把两个前缀都当宏起点。
//!
//! # 为什么在**文本层**做，而不是在词法层加 token
//!
//! 因为原文说的位置是"**任何位置**"，而且替换必须发生在**建树之前** —— 在词法层
//! 加 token 只能覆盖"表达式里"，覆盖不到 SQL 文本与属性表达式；而且 `%` 在词法层
//! 已经是 [`crate::expr::ast::BinOp::Mod`]，`$` 又是 `$B2` 绝对引用的前缀，
//! 硬塞进 token 集就要改两处优先级表。**纯文本前置替换把这两个冲突一起消掉**：
//! 替换之后交给现成的 [`crate::expr::parse_with`]，词法层一行都不用动。
//!
//! # ⛔ 与三个"长得很像"的东西的区别（别混）
//!
//! | 机制 | 记号 | 谁处理 | 本模块管吗 |
//! |------|------|--------|-----------|
//! | **宏** | `${名}` / `%{名}` | 本模块（建树前文本替换） | ✅ |
//! | 参数 | `@名` | [`crate::params::ParamTable`]（建树期**内联成 `Constant(值)`**） | ❌ |
//! | 宏参数 | `@@名` | 同上（`Expression.java:183-186`，**不查** `isParamName`） | ❌ |
//! | JS 模板参数 | `${参数名}` | ECharts 模板生成器（`034_mbcslx81` 的三个固定参数 `id`/`width`/`height`） | ❌ |
//!
//! ⚠ 最后一行与宏**记号相同**但是两套东西：那是**图形模板**里的 JS 插值，发生在
//! 生成 JS 的阶段，不在报表表达式的解析链上。本模块只被"表达式 / SQL 文本"调用，
//! 不碰图形模板 —— 落点见 [`substitute`] 的调用方注释。
//!
//! # 替换规则（三条，每条都是判据）
//!
//! 1. **递归展开**：宏值里可以再引用宏（`A = "${B}-1"`），一直替换到没有宏引用为止。
//! 2. **未登记 ⇒ 显式报错**（[`crate::error::EngineError::UnknownMacro`]）—— 与
//!    `@p` 未登记时报 `UnknownParam` 同一条纪律：**不静默**。静默留着 `${x}` 会让
//!    它在词法层被拆成 `$` + `{x}`，报出一个与真实原因无关的语法错。
//! 3. **成环 ⇒ 显式报错**，用引用栈检查环并限制嵌套深度（见 [`MAX_ROUNDS`]）。
//!
//! ⛔ **没有转义记号**：写不出"字面的 `${`"。产品原文没给转义，本模块**不发明**；
//! 真需要时由调用方先替换（这也是"不猜"纪律的一部分）。

use std::collections::BTreeMap;

use crate::error::{EngineError, Result};

/// 宏引用的最大嵌套深度。独立宏的数量不受此值限制。
pub const MAX_ROUNDS: usize = 64;

/// 报表宏表（`MacroResolver2` 查的那张表）。
///
/// 值是**字符串**（原文："宏代表没有数据类型的一个字符串"），不是 [`crate::value::Value`]
/// —— 与 [`crate::params::ParamTable`] 的关键差别就在这里：参数是**带类型的值**、
/// 建树期内联成 `Constant(值)`；宏是**文本**、建树前就把自己从串里抹掉。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MacroTable {
    entries: BTreeMap<String, String>,
}

impl MacroTable {
    /// 本项目写入 RPX 自定义属性的宏。产品私有 MacroMetaData 保持原始字节。
    pub fn from_report(rd: &rpx_core::ReportDefine) -> Self {
        rd.custom_properties
            .iter()
            .filter_map(|(key, value)| {
                Some((
                    key.strip_prefix("rust_report.macro.")?.to_string(),
                    value.as_ref()?.clone(),
                ))
            })
            .collect()
    }
    pub(crate) fn overlay(&mut self, other: &Self) {
        self.entries.extend(other.entries.clone())
    }
    /// 空表 —— 没有任何宏登记。出现宏引用时仍报 UnknownMacro。
    pub const EMPTY: MacroTable = MacroTable {
        entries: BTreeMap::new(),
    };

    /// 空表（惯用构造器形式）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一个宏（同名重复登记**覆盖**，与 `Map.put` 一致）。
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.entries.insert(name.into(), value.into());
    }

    /// 名字是否登记。
    pub fn is_macro(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// 取宏值；未登记返回 `None`。
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries.get(name).map(String::as_str)
    }

    /// 登记个数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 空表判定。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl FromIterator<(String, String)> for MacroTable {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        MacroTable {
            entries: iter.into_iter().collect(),
        }
    }
}

/// 一次扫描的结果：找到了下一个宏引用，或"没有宏引用了"。
struct Hit {
    /// 宏前缀的起始字节下标（`$` 或 `%`）。
    start: usize,
    /// `}` 的字节下标。
    end: usize,
    /// 花括号里的名字（逐字，不去空白）。
    name: String,
}

/// 找 `src` 里**第一个**宏引用（`${` 或 `%{` 先出现者）。
///
/// 找不到 ⇒ `Ok(None)`；找到但缺 `}` ⇒ `Err(MalformedMacro)`。
fn find_first(src: &str) -> Result<Option<Hit>> {
    let bytes = src.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        let is_head = (bytes[i] == b'$' || bytes[i] == b'%') && bytes[i + 1] == b'{';
        if !is_head {
            i += 1;
            continue;
        }
        let brace = i + 1;
        let Some(close) = src[brace + 1..].find('}') else {
            return Err(EngineError::MalformedMacro {
                detail: format!("{} 处的宏引用没有闭合的 `}}`", &src[i..]),
            });
        };
        let end = brace + 1 + close;
        let name = src[brace + 1..end].to_string();
        return Ok(Some(Hit {
            start: i,
            end,
            name,
        }));
    }
    Ok(None)
}

/// 替换 `src` 里的全部宏引用（`${名}` 与 `%{名}`）。
///
/// 返回**替换后**的字符串；未登记宏 / 未闭合 / 成环都返回 `Err`（见模块文档的三条规则）。
///
/// ⛔ 调用点必须**在建树之前**（`crate::expr::parse_with` 的上游）—— 替换后的串才
/// 是词法层的输入。落在建树之后等于没做（词法层不认识 `${`）。
pub fn substitute(src: &str, macros: &MacroTable) -> Result<String> {
    expand(src, macros, &mut Vec::new())
}

fn expand(src: &str, macros: &MacroTable, stack: &mut Vec<String>) -> Result<String> {
    let mut out = String::new();
    let mut rest = src;
    while let Some(hit) = find_first(rest)? {
        if hit.name.is_empty() {
            return Err(EngineError::MalformedMacro {
                detail: "宏名为空".into(),
            });
        }
        if stack.contains(&hit.name) || stack.len() >= MAX_ROUNDS {
            return Err(EngineError::MalformedMacro {
                detail: format!("宏引用成环或嵌套超过 {MAX_ROUNDS} 层：{}", hit.name),
            });
        }
        let value = macros
            .get(&hit.name)
            .ok_or_else(|| EngineError::UnknownMacro {
                name: hit.name.clone(),
            })?;
        stack.push(hit.name);
        let value = expand(value, macros, stack)?;
        stack.pop();
        if out.len() + hit.start + value.len() > 16 * 1024 * 1024 {
            return Err(EngineError::MalformedMacro {
                detail: "宏展开结果超过 16 MiB".into(),
            });
        }
        out.push_str(&rest[..hit.start]);
        out.push_str(&value);
        rest = &rest[hit.end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(pairs: &[(&str, &str)]) -> MacroTable {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn an_empty_table_rejects_references_but_preserves_plain_text() {
        assert_eq!(substitute("A1+1", &MacroTable::EMPTY).unwrap(), "A1+1");
        assert!(matches!(
            substitute("${x}", &MacroTable::EMPTY),
            Err(EngineError::UnknownMacro { .. })
        ));
    }

    #[test]
    fn many_independent_references_are_not_a_cycle() {
        let t = table(&[("a", "1")]);
        assert_eq!(
            substitute(&"${a}".repeat(100), &t).unwrap(),
            "1".repeat(100)
        );
    }

    #[test]
    fn both_sigils_are_accepted() {
        let t = table(&[("p", "ds1.qty"), ("q", "B2")]);
        assert_eq!(substitute("sum(${p})", &t).unwrap(), "sum(ds1.qty)");
        assert_eq!(substitute("sum(%{p})", &t).unwrap(), "sum(ds1.qty)");
        assert_eq!(substitute("${q}+%{q}", &t).unwrap(), "B2+B2");
    }

    #[test]
    fn substitution_is_purely_textual_and_reaches_inside_literals() {
        // 产品 `replaceMacros` 是**整串**替换（建表达式之前），不区分字符串字面量内部。
        let t = table(&[("who", "张三")]);
        assert_eq!(substitute("\"你好 ${who}\"", &t).unwrap(), "\"你好 张三\"");
    }

    #[test]
    fn a_macro_value_may_reference_another_macro() {
        let t = table(&[("a", "${b}+1"), ("b", "B2*2")]);
        assert_eq!(substitute("${a}", &t).unwrap(), "B2*2+1");
    }

    #[test]
    fn an_unknown_macro_is_an_explicit_error() {
        let t = table(&[("p", "1")]);
        let err = substitute("${nope}", &t).unwrap_err();
        assert!(
            matches!(err, EngineError::UnknownMacro { ref name } if name == "nope"),
            "{err}"
        );
        assert!(err.to_string().contains("nope"), "{err}");
    }

    #[test]
    fn an_unclosed_reference_is_an_explicit_error() {
        let t = table(&[("p", "1")]);
        let err = substitute("A1+${p", &t).unwrap_err();
        assert!(matches!(err, EngineError::MalformedMacro { .. }), "{err}");
    }

    #[test]
    fn an_empty_name_is_an_explicit_error() {
        let t = table(&[("p", "1")]);
        let err = substitute("%{}", &t).unwrap_err();
        assert!(matches!(err, EngineError::MalformedMacro { .. }), "{err}");
    }

    #[test]
    fn a_cycle_is_an_explicit_error_not_a_hang() {
        let t = table(&[("a", "${b}"), ("b", "${a}")]);
        let err = substitute("${a}", &t).unwrap_err();
        assert!(matches!(err, EngineError::MalformedMacro { .. }), "{err}");
        assert!(err.to_string().contains("成环"), "{err}");
    }

    #[test]
    fn a_self_reference_is_also_a_cycle() {
        let t = table(&[("a", "x${a}")]);
        assert!(substitute("${a}", &t).is_err());
    }

    #[test]
    fn insert_overwrites_like_a_java_hash_map() {
        let mut t = MacroTable::new();
        t.insert("p", "1");
        t.insert("p", "2");
        assert_eq!(t.get("p"), Some("2"));
        assert_eq!(t.len(), 1);
        assert!(t.is_macro("p") && !t.is_macro("q"));
    }

    #[test]
    fn the_first_reference_wins_when_both_sigils_appear() {
        // `%{` 在前 ⇒ 先替换它；左到右单趟扫描的必然结果。
        let t = table(&[("a", "A"), ("b", "B")]);
        assert_eq!(substitute("%{a}+${b}", &t).unwrap(), "A+B");
    }
}
