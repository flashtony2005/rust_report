//! 参数串切分与调用组装 —— 从 `parser.rs` 拆出，因为这两件事**不依赖 Pratt 主循环**。
//!
//! 算法正文见 `docs/m2-engine-design.md` §13.2（`Call` 节点）与 §13.5（未支持清单）。

use crate::error::Result;
use crate::value::Value;

use super::ast::{Call, Expr, ParamKind};
use super::classify::{split_ds, split_top, top_level_kind};
use super::names::{classify_bare, classify_ds};
use super::parser::parse_tokens;
use super::token::Tok;

/// 切分一个参数串（`ParamParser._$1:35-64` 的简化版）。
///
/// ⚠ **只建模到顶层一层**：返回顶层类别 + 顶层切出的子表达式。
/// 完整的嵌套 `IParam` 树（`;` 里套 `,` 里套 `:`）留给定稿求值期（§13.5 已标注）。
pub fn parse_params(toks: &[Tok]) -> Result<(ParamKind, Vec<Expr>)> {
    if toks.is_empty() {
        // 空参数串 ⇒ `ParamParser.parse` 返回 null（`ParamParser.java:39-41`）
        return Ok((ParamKind::Leaf, Vec::new()));
    }
    let kind = top_level_kind(toks);
    let sep = match kind {
        ParamKind::Semicolon => Tok::Semi,
        ParamKind::Comma => Tok::Comma,
        ParamKind::Colon => Tok::Colon,
        ParamKind::Leaf => return Ok((ParamKind::Leaf, vec![parse_tokens(toks)?])),
    };
    let mut args = Vec::new();
    for piece in split_top(toks, &sep) {
        if piece.is_empty() {
            // 空子段 ⇒ 产品返回 null IParam（ArgumentTokenizer 的相邻分隔符）
            args.push(Expr::Const(Value::Null));
        } else {
            args.push(parse_tokens(piece)?);
        }
    }
    Ok((kind, args))
}

/// 组装一次调用：切 `@选项`、判 DS 限定、查名字表。
pub fn build_call(word: String, param_kind: ParamKind, args: Vec<Expr>) -> Expr {
    // `名@选项`（Expression.java:212-218 的 `indexOf('@')`）
    let (head, option) = match word.split_once('@') {
        Some((h, o)) => (h.to_string(), Some(o.to_string())),
        None => (word, None),
    };
    if let Some((qualifier, name)) = split_ds(&head) {
        let kind = classify_ds(&name);
        return Expr::Call(Call {
            qualifier: Some(qualifier),
            name,
            option,
            kind,
            param_kind,
            args,
        });
    }
    // `list(...)` 登记为 `ValueList`（`FunctionLib.java:165`），不是普通函数
    if head == "list" {
        return Expr::ValueList {
            kind: param_kind,
            items: args,
        };
    }
    let kind = classify_bare(&head);
    Expr::Call(Call {
        qualifier: None,
        name: head,
        option,
        kind,
        param_kind,
        args,
    })
}
