//! 参数串切分与调用组装 —— 从 `parser.rs` 拆出，因为这两件事**不依赖 Pratt 主循环**。
//!
//! 算法正文见 `docs/m2-engine-design.md` §13.2（`Call` 节点）与 §13.5（未支持清单）。
//!
//! # M5-01：从「只建模顶层」到完整 `IParam` 树
//!
//! [`parse_params`] 仍保留**顶层一层**的扁平视图（`list(...)`/`[...]`/普通函数
//! 都够用）；[`build_param`] 另建**完整嵌套树**（产品 `ParamParser._$1` 的递进
//! 复刻），由 [`build_call`] 只挂到 `ds.名(...)` 调用上（[`Call::param_tree]`）——
//! `ds.select` 的参数矩阵（`,` 里套 `:`）是唯一消费者，语义见
//! `docs/spec-m5.md` §2.2 与 [`crate::expr::dsselect`]。

use crate::error::Result;
use crate::params::ParamTable;
use crate::value::Value;

use super::ast::{Call, Expr, Param, ParamKind};
use super::classify::{has_top_sep, split_ds, split_top, top_level_kind};
use super::names::{classify_bare, classify_ds};
use super::parser::parse_tokens_with;
use super::token::Tok;

/// 切分一个参数串（`ParamParser._$1:35-64` 的简化版）。
///
/// ⚠ **只建模到顶层一层**：返回顶层类别 + 顶层切出的子表达式。
/// 完整的嵌套 `IParam` 树（`;` 里套 `,` 里套 `:`）留给定稿求值期（§13.5 已标注）。
///
/// `params` 透传给每个子段的解析（M3-01）：`sum(@p)`、`f(@a,@b)` 这类形态
/// 的参数绑定发生在**子段**里，漏传会让 `@p` 误报 `UnknownParam`。
pub fn parse_params(toks: &[Tok], params: &ParamTable) -> Result<(ParamKind, Vec<Expr>)> {
    if toks.is_empty() {
        // 空参数串 ⇒ `ParamParser.parse` 返回 null（`ParamParser.java:39-41`）
        return Ok((ParamKind::Leaf, Vec::new()));
    }
    let kind = top_level_kind(toks);
    let sep = match kind {
        ParamKind::Semicolon => Tok::Semi,
        ParamKind::Comma => Tok::Comma,
        ParamKind::Colon => Tok::Colon,
        ParamKind::Leaf => return Ok((ParamKind::Leaf, vec![parse_tokens_with(toks, params)?])),
    };
    let mut args = Vec::new();
    for piece in split_top(toks, &sep) {
        if piece.is_empty() {
            // 空子段 ⇒ 产品返回 null IParam（ArgumentTokenizer 的相邻分隔符）
            args.push(Expr::Const(Value::Null));
        } else {
            args.push(parse_tokens_with(piece, params)?);
        }
    }
    Ok((kind, args))
}

/// 组装一次调用：切 `@选项`、判 DS 限定、查名字表。
///
/// `tree` = [`build_param`] 产出的嵌套参数树；只有 `ds.名(...)`（`qualifier` 非空）
/// 会带进 [`Call::param_tree]` —— `ds.select` 的参数矩阵（M5-01）是唯一消费者。
pub fn build_call(
    word: String,
    param_kind: ParamKind,
    args: Vec<Expr>,
    tree: Option<Box<Param>>,
) -> Expr {
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
            param_tree: tree,
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
        // 普通函数不走 DS 参数矩阵 ⇒ 不建树（树由 atom 只对 DS 调用生成，
        // 这里兜底置空，双保险）。
        param_tree: None,
    })
}

/// `ParamParser._$1`（`ParamParser.java:35-64`）的完整复刻 —— 建嵌套 `IParam` 树。
///
/// 层级递进 `prev`（产品 `prevLevel`）：`None('\0') → ';' → ',' → ':'`；
/// 每档问一句「本层分隔符在顶层出现没有」（[`has_top_sep`]，产品
/// `ParamParser._$1(str, level):49-57`），出现就切分出本层节点、子段按
/// `prev=本层` 递归；都不出现 ⇒ 叶子（`:53-54`）。`':'` 层不再下探
/// （`:42-47`）：本层就是表达式。
///
/// 空 token 串 → `None`（产品 `:39-41`：trim 后长度 0 返回 null ——
/// `select(item, , , item)` 的空段由此变成 null `IParam`，`getSub(i)` 取到 null）。
///
/// 内部递归用裸 [`Param`]（`Node.subs` 的 `Vec` 已是间接层）；只有在挂上
/// [`Call::param_tree]` 时才 Box 断环（E0072）—— 由调用方 `map(Box::new)` 完成。
pub fn build_param(
    toks: &[Tok],
    params: &ParamTable,
    prev: Option<ParamKind>,
) -> Result<Option<Param>> {
    if toks.is_empty() {
        return Ok(None);
    }
    // 本层之后的递进档位（产品 `_$1(prevLevel):66-79` 的 switch）
    let levels: &[ParamKind] = match prev {
        None => &[ParamKind::Semicolon, ParamKind::Comma, ParamKind::Colon],
        Some(ParamKind::Semicolon) => &[ParamKind::Comma, ParamKind::Colon],
        Some(ParamKind::Comma) => &[ParamKind::Colon],
        // `:` 层（与不可能的 Leaf）直接当叶子，不再下探
        Some(ParamKind::Colon) | Some(ParamKind::Leaf) => &[],
    };
    for &level in levels {
        let sep = match level {
            ParamKind::Semicolon => Tok::Semi,
            ParamKind::Comma => Tok::Comma,
            ParamKind::Colon => Tok::Colon,
            ParamKind::Leaf => unreachable!("递进档位不含 Leaf"),
        };
        if !has_top_sep(toks, &sep) {
            continue;
        }
        let mut subs = Vec::new();
        for piece in split_top(toks, &sep) {
            subs.push(build_param(piece, params, Some(level))?);
        }
        return Ok(Some(Param::Node { kind: level, subs }));
    }
    Ok(Some(Param::Leaf(parse_tokens_with(toks, params)?)))
}
