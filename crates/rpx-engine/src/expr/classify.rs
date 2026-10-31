//! 原子与运算符的**分类表** —— 语法层的纯查表辅助，不含递归。
//!
//! 算法正文见 `docs/m2-engine-design.md` §13.1（token/优先级）与 §13.5（未支持清单）。
//!
//! 每个函数都点名了它在产品源码里的对应物，**不是**"顺手写的工具函数"。

use crate::error::EngineError;
use crate::value::Value;

use super::ast::{BinOp, ParamKind};
use super::token::Tok;

/// 统一的"不支持"错误出口（复用 `EngineError::UnsupportedExpr`，`error.rs:23`）。
pub fn err(detail: String) -> EngineError {
    EngineError::UnsupportedExpr { expr: detail }
}

/// 中缀运算符表：`(运算符, 绑定力)`。绑定力逐条取自各运算符类的 `this.priority`（见 §13.1）。
///
/// 所有中缀运算符都是**左结合**：产品的 `Expression.create`（`:442`）用严格 `<` 比较
/// 沿右脊下探，等价于 `(lbp, rbp) = (pri, pri + 1)`。
pub fn infix(t: &Tok) -> Option<(BinOp, u8)> {
    let pair = match t {
        // Comma.java:16 / Evaluate.java:18 / Link.java:25
        Tok::Comma => (BinOp::Comma, 1),
        Tok::Eq => (BinOp::Eval, 2),
        Tok::Colon => (BinOp::Link, 4),
        // Or.java:17 / And.java:18（`||` `&&` 与关键字同优先级）
        Tok::PipePipe => (BinOp::Or, 5),
        Tok::AmpAmp => (BinOp::And, 6),
        // Equals/NotEquals/In：优先级 10
        Tok::EqEq => (BinOp::Eq, 10),
        Tok::BangEq => (BinOp::Ne, 10),
        // Greater/NotSmaller/Smaller/NotGreater：优先级 11
        Tok::Gt => (BinOp::Gt, 11),
        Tok::Ge => (BinOp::Ge, 11),
        Tok::Lt => (BinOp::Lt, 11),
        Tok::Le => (BinOp::Le, 11),
        // Add/Subtract：优先级 13；Multiply/Divide/Mod：优先级 14
        Tok::Plus => (BinOp::Add, 13),
        Tok::Minus => (BinOp::Sub, 13),
        Tok::Star => (BinOp::Mul, 14),
        Tok::Slash => (BinOp::Div, 14),
        Tok::Percent => (BinOp::Mod, 14),
        // 关键字式运算符（`Expression.java:157-168`，equalsIgnoreCase）
        Tok::Word(w) if w.eq_ignore_ascii_case("or") => (BinOp::Or, 5),
        Tok::Word(w) if w.eq_ignore_ascii_case("and") => (BinOp::And, 6),
        Tok::Word(w) if w.eq_ignore_ascii_case("in") => (BinOp::In, 10),
        _ => return None,
    };
    Some(pair)
}

/// 是否是格号：`字母+ 后接 数字+`（如 `B2` / `AA10`）。**不校验行列范围**（那要求 Context）。
///
/// 对应产品的 `CellLocation.parse`（`Expression.java:195,201` 的 `getSource(id)` 键格式）。
/// 本函数只做形态判断：`B2` 是，`sum`/`ds1`/`1.5`/`B2B3` 都不是。
pub fn is_cell_id(w: &str) -> bool {
    let (mut has_alpha, mut has_digit, mut in_digits) = (false, false, false);
    for c in w.chars() {
        if c.is_ascii_alphabetic() {
            if in_digits {
                return false;
            }
            has_alpha = true;
        } else if c.is_ascii_digit() {
            has_digit = true;
            in_digits = true;
        } else {
            return false;
        }
    }
    has_alpha && has_digit
}

/// 按**最后一个** `.` 切出 `(数据集, 列/函数名)`。对应 `Expression.java:174` 的 `id.lastIndexOf(46)`。
pub fn split_ds(w: &str) -> Option<(String, String)> {
    let i = w.rfind('.')?;
    if i == 0 || i + 1 >= w.len() {
        return None;
    }
    let head = &w[..i];
    // ⚠ 数字字面量守卫（2026-10-16 补，`ds[i]` 下标单测踩出）：`1.5` 的 head
    // "1" 是纯数字 ⇒ 不是 ds 名。产品口径是「存在才切」——
    // `ctx.getDataSet(dsName) != null`（`Expression.java:174-176`）；本仓 parse 期
    // 无 ds 清单（与 CellRef provisional 同源的形态策略），取「点号前纯数字 ⇒
    // 不切」的近似：合法 ds 名不会以纯数字作前缀，其余歧义由 eval 期
    // `MissingDataset` 兜底。修前 `parse("1.5")` 会错切出 `DsColumn{"1","5"}`。
    if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((head.to_string(), w[i + 1..].to_string()))
}

/// 字面量解析 —— 逐条对齐 `Variant2.parse`（`Variant2.java:748-797`）。
///
/// 顺序：`null` → `true`/`false` → `0x` 十六进制（失败则**继续**按十进制试，`:781-788` 的
/// catch 后不返回）→ `Integer` → `Long` → `Double`。
/// 产品的最后一步是 `parseDate`（`Variant2.java:795`），**本切片不实现** → 返回 `None`
/// （调用方落成 [`super::ast::Expr::Unknown`]，`is_extend()` 同为 false）。
pub fn literal(w: &str) -> Option<Value> {
    if w.eq_ignore_ascii_case("null") {
        return Some(Value::Null);
    }
    if w.eq_ignore_ascii_case("true") {
        return Some(Value::Bool(true));
    }
    if w.eq_ignore_ascii_case("false") {
        return Some(Value::Bool(false));
    }
    if let Some(hex) = w.strip_prefix("0x").or_else(|| w.strip_prefix("0X")) {
        if let Ok(v) = i64::from_str_radix(hex, 16) {
            return Some(Value::Long(v));
        }
    }
    if let Ok(v) = w.parse::<i32>() {
        return Some(Value::Int(v));
    }
    if let Ok(v) = w.parse::<i64>() {
        return Some(Value::Long(v));
    }
    if let Ok(v) = w.parse::<f64>() {
        return Some(Value::Double(v));
    }
    None
}

/// 参数/值列表的**顶层分隔符类别** —— 对齐 `ParamParser._$1:66-79` 的层级
/// `'\0' → ';' → ',' → ':'`：取**第一个出现**的那一级；都不出现则为叶子。
///
/// 只统计**顶层**（跳过 `(`/`[`/`{` 内部），对应 `ParamParser._$1(str, sep):81-129`
/// 里对引号与括号的跳过。
pub fn top_level_kind(toks: &[Tok]) -> ParamKind {
    let mut depth = 0i32;
    let (mut has_comma, mut has_colon) = (false, false);
    for t in toks {
        match t {
            Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
            Tok::RParen | Tok::RBracket | Tok::RBrace => depth -= 1,
            Tok::Semi if depth == 0 => return ParamKind::Semicolon,
            Tok::Comma if depth == 0 => has_comma = true,
            Tok::Colon if depth == 0 => has_colon = true,
            _ => {}
        }
    }
    if has_comma {
        ParamKind::Comma
    } else if has_colon {
        ParamKind::Colon
    } else {
        ParamKind::Leaf
    }
}

/// 按 `sep` 在**顶层**切分。对应 `ArgumentTokenizer(paramStr, level)`（`ParamParser.java:59`）。
pub fn split_top<'a>(toks: &'a [Tok], sep: &Tok) -> Vec<&'a [Tok]> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
            Tok::RParen | Tok::RBracket | Tok::RBrace => depth -= 1,
            _ if depth == 0 && t == sep => {
                out.push(&toks[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&toks[start..]);
    out
}

/// 顶层是否出现分隔符 `sep` —— `ParamParser._$1(str, separator):81-129` 的
/// **存在性判定**（跳过引号与 `(`/`[`/`{` 内部；与 [`top_level_kind`] 同一套深度规则）。
///
/// M5-01：[`super::call::build_param`] 的层级递进要逐档问「本层分隔符出现没有」，
/// 而 [`top_level_kind`] 只返回第一个出现的那一档，答不了这个问。
pub fn has_top_sep(toks: &[Tok], sep: &Tok) -> bool {
    let mut depth = 0i32;
    for t in toks {
        match t {
            Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
            Tok::RParen | Tok::RBracket | Tok::RBrace => depth -= 1,
            _ if depth == 0 && t == sep => return true,
            _ => {}
        }
    }
    false
}
