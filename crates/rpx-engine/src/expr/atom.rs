//! 原子识别 —— 一个 `Word`/`Str`/括号组到底变成了哪个 AST 节点。
//!
//! 从 `parser.rs` 拆出来的理由：Pratt 主循环只管"**什么时候**结合"，而"结合成什么"
//! 是另一类问题（要查名字表、要判字面量、要按产品的 8 步顺序试）。两者分开后
//! [`super::parser::Parser`] 只剩结合逻辑，本文件集中承载
//! `Expression.createNode`（`Expression.java:156-239`）那条判定链。
//!
//! 算法正文见 `docs/m2-engine-design.md` §13.2（节点清单）。

use crate::error::{EngineError, Result};
use crate::value::Value;

use super::ast::{Expr, UnOp};
use super::call::{build_call, build_param, parse_params};
use super::classify::{err, is_cell_id, literal, split_ds};
use super::lexer::unescape;
use super::parser::Parser;
use super::token::Tok;

impl<'a> Parser<'a> {
    /// 原子：字面量 / 格引用 / 参数 / 调用 / 数据集列 / 未知符号。
    ///
    /// 判定顺序**逐条对齐产品 `Expression.createNode`**（`Expression.java:156-239`）：
    /// `$[...]`(:169) → `@参数`(:178) → `$格号`(:193) → `数据集.名`(:174,:204) →
    /// **格引用**(:201) → `名字(`(:210) → 字面量(:235) → `UnknownSymbol`(:237)。
    /// 顺序本身就是语义：`$B2(x)` 是格引用而非调用，`ds1.qty` 不是格号。
    pub(super) fn parse_primary(&mut self) -> Result<Expr> {
        let w = match self.peek().clone() {
            Tok::Str(s) => {
                self.bump();
                return Ok(Expr::Const(Value::Text(s)));
            }
            Tok::Word(w) => {
                self.bump();
                w
            }
            other => return Err(err(format!("无法把 {} 当作表达式原子", other.label()))),
        };
        // 关键字式中缀运算符不能作原子（`Expression.java:157-168` 只在“取节点”时识别）
        if w.eq_ignore_ascii_case("and")
            || w.eq_ignore_ascii_case("or")
            || w.eq_ignore_ascii_case("in")
        {
            return Err(err(format!("`{w}` 不能作为表达式原子")));
        }
        // ⛔ `$[...]` **不是**格引用：`Expression.java:169-172` 把它变成
        //    `Constant(Escape.remove(内文.trim()))` —— 一个字符串常量。
        //    （`$` 会单独成一个 `Word`，因为 `[` 是分隔符；见 token.rs 的 `is_delimiter`。）
        if w == "$" {
            self.expect(Tok::LBracket)?;
            let inner = self.take_group(Tok::RBracket)?;
            return Ok(literal_slice_to_const(inner));
        }
        // `@参数` / `@@宏参数` / `@`（Expression.java:178-192）
        //
        // M3-01：这里从"建 `Expr::Param` 占位"改成**建树期内联**——与产品
        // `Constant(getParamValue(id))` 同构。证据：`Expression.java:185`（`@@`）
        // / `:188`（`@`）建的都是带值 `Constant`；三种边界的真机行为见
        // `tests/fixtures/probe_param_full_out.utf8.txt` 与 RUNLOG §24.1。
        if let Some(rest) = w.strip_prefix('@') {
            if rest.is_empty() {
                return Ok(Expr::CurrentCell);
            }
            return Ok(match rest.strip_prefix('@') {
                // ⛔ `@@p` **不查** `isParamName`（`:183-186` 无条件取参值）；
                // 未登记 ⇒ `getParamValue` 返回 null ⇒ `Constant(null)`，
                // **不报错**（真机证实：RUNLOG §24.1，计算态 extend=17、格值 null）。
                Some(name) => Expr::Const(self.params.get(name).cloned().unwrap_or(Value::Null)),
                // `@p` 要过 `ctx.isParamName`（`:187`）：登记（**含值为 null**）⇒
                // `Constant(值)`；未登记 ⇒ `ReportError(unknownParam)`（`:189-191`）。
                None => match self.params.get(rest) {
                    Some(v) => Expr::Const(v.clone()),
                    None => {
                        return Err(EngineError::UnknownParam {
                            name: rest.to_string(),
                        })
                    }
                },
            });
        }
        // `$` 整格绝对引用（Expression.java:193-200）
        if let Some(rest) = w.strip_prefix('$') {
            if rest.is_empty() {
                return Err(err("裸 `$` 后必须跟格号".to_string()));
            }
            return Ok(Expr::CellRef {
                id: rest.to_string(),
                absolute: true,
                provisional: true,
            });
        }
        // `数据集.列`：DS 限定路径在产品里排在格引用**之前**（`Expression.java:174`
        // 早于 `:201`），所以形态像格号的 `ds1.qty` 不会被 cell 分支吞掉。
        // ⚠ 后接 `(` 时走 [`build_call`]（`ds1.sum(...)` 是 DS 函数，不是列引用）。
        if !matches!(self.peek(), Tok::LParen) {
            if let Some((ds, col)) = split_ds(&w) {
                return Ok(Expr::DsColumn { ds, col });
            }
        }
        // ⛔ **格号是「查出来的」不是「看形状判的」**：产品是
        //    `((ExtCellSet)cs).getSource(id) != null`（`Expression.java:201`），
        //    查不到就顺次落到 DS 列 / 参数名(`:232`) / 函数(`:210`) / 字面量(`:235`) /
        //    UnknownSymbol(`:237`)。本切片没有引擎矩阵，只能**暂定**按形态建 CellRef
        //    并打 `provisional` 标记 —— `qty1` / `amount2` 这类**数据集列名会被误判**。
        //    对 `is_extend()` **无影响**（CSVariable 与 UnknownSymbol 都是 false，见
        //    `CSVariable.java:379-381` 与 `Node.java:82`），但求值期（M2-06）**必须**
        //    拿引擎矩阵重新解析。未验证 ⇒ §13.5 已登记。
        if is_cell_id(&w) {
            return Ok(Expr::CellRef {
                id: w,
                absolute: false,
                provisional: true,
            });
        }
        // 函数调用（裸名先查普通表；DS 回退因 ds==null 不可达，见 names.rs）
        if matches!(self.peek(), Tok::LParen) {
            self.bump();
            let inner = self.take_group(Tok::RParen)?;
            let (param_kind, args) = parse_params(inner, self.params)?;
            // M5-01：DS 调用另建完整嵌套参数树（`ds.select` 参数矩阵的唯一载体；
            // 普通函数/`list` 的扁平 args 即全部，树为 None）。Box 断环见
            // `Call::param_tree`（E0072）。
            let tree = if split_ds(&w).is_some() {
                build_param(inner, self.params, None)?.map(Box::new)
            } else {
                None
            };
            return Ok(build_call(w, param_kind, args, tree));
        }
        // 裸名参数（`Expression.java:232-234`）：`ctx.isParamName(id)` ⇒
        // `ParamNode(id, ctx.getParamValue(id))`。`ParamNode extends Constant`
        // （`ParamNode.java:3`）⇒ 与 `@p` 的 `Constant(值)` 同构，内联为 [`Expr::Const`]。
        //
        // ⚠ 位置是契约：排在格号形状判定（`:201`）与函数判定（`:210`）**之后** ——
        // 名字同时是函数名与参数名时**函数优先**（`isFunName` 先判，`:217`）。
        //
        // ⚠ **已知限制（provisional 策略的推论，2026-10-14 登记）**：名字形如
        // `[A-Za-z]+\d+`（`p1`、`t1`）且**不是矩阵里真格号**时，产品落到本分支，
        // 而本 crate 的形状判定 [`is_cell_id`] 已在上面截胡判成 `CellRef`。
        // 与 `qty1` 这类数据集列名的误判**同类同源**，等 M2-06 的矩阵重解析一并修正；
        // 现有坏后果只落在"参数名形如格号且值为 List"（少一次扩展）。见 `params.rs` 文档。
        if let Some(v) = self.params.get(&w) {
            return Ok(Expr::Const(v.clone()));
        }
        // 字面量（Variant2.parse:748-797）；落空 → UnknownSymbol（:235-238）
        if let Some(v) = literal(&w) {
            return Ok(Expr::Const(v));
        }
        Ok(Expr::Unknown(w))
    }

    /// `[...]` 值列表 / 区间（`Expression.java:136-155`，preNode 为空时的 `ValueList` 分支）。
    pub(super) fn parse_value_list(&mut self) -> Result<Expr> {
        self.bump(); // 吃掉 `[`
        let inner = self.take_group(Tok::RBracket)?;
        let (kind, items) = parse_params(inner, self.params)?;
        Ok(Expr::ValueList { kind, items })
    }

    /// `{...}` 单元格选择（`Expression.java:125-135` 的 `Select`）。
    ///
    /// ⛔ 产品的 `Select` **只拿到 parameter**（`Expression.java:132` 的 `setParameter`），
    /// 花括号里的表达式是**参数**、不是 `left`。`isExtended()` 读的是 `left`
    /// （`SpecialFunction.java:27-32`），所以 `arg` 对扩展性**没有贡献** ——
    /// 详见 [`super::ast::Expr::is_extend`] 的 `Select` 分支。
    pub(super) fn parse_select(&mut self) -> Result<Expr> {
        self.bump(); // 吃掉 `{`
        let inner = self.take_group(Tok::RBrace)?;
        let arg = if inner.is_empty() {
            Expr::Const(Value::Null)
        } else {
            super::parser::parse_tokens_with(inner, self.params)?
        };
        Ok(Expr::Select { arg: Box::new(arg) })
    }
}

/// `$[...]` 的字面内容 → 字符串常量（`Expression.java:171`）。
fn literal_slice_to_const(inner: &[Tok]) -> Expr {
    Expr::Const(Value::Text(unescape(join_tokens(inner).trim())))
}

/// 把 token 片段拼回**源文本**（`$[...]` 要按原文取内容，见上）。
fn join_tokens(toks: &[Tok]) -> String {
    let mut out = String::new();
    for (i, t) in toks.iter().enumerate() {
        if i > 0 && needs_space(&toks[i - 1], t) {
            out.push(' ');
        }
        match t {
            Tok::Word(w) | Tok::Str(w) => out.push_str(w),
            other => out.push_str(other.symbol()),
        }
    }
    out
}

/// 两个相邻 token 之间是否补空格：`[B 2]` 要还原成 `B 2`，而 `(a)` 不必补。
fn needs_space(prev: &Tok, cur: &Tok) -> bool {
    let word_like = |t: &Tok| matches!(t, Tok::Word(_) | Tok::Str(_) | Tok::LParen | Tok::LBracket);
    word_like(prev) && word_like(cur)
}

/// 前缀运算符的构造糖（`Not`/`Negative`/`Plus`/`Address` 都只挂 `right`）。
pub(super) fn unary(op: UnOp, rhs: Expr) -> Expr {
    Expr::Unary {
        op,
        rhs: Box::new(rhs),
    }
}
