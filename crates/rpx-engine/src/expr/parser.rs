//! 表达式语法分析 —— 手写 **Pratt（运算符优先级）解析器**。
//!
//! 算法正文见 `docs/m2-engine-design.md` §13.1（优先级表）与 §13.2（节点清单）。
//!
//! 本文件只管**什么时候结合**（结合力）；「结合成什么节点」在 [`super::atom`]，
//! 参数切分与调用组装在 [`super::call`]。token 类型在 [`super::token`]，扫描在 [`super::lexer`]。
//!
//! # 为什么是 Pratt，而不是照抄产品的线性扫描
//!
//! 产品的 `Expression.create`（`Expression.java:289-456`）是一趟**线性扫描 + 右脊下探**的
//! 运算符优先级解析器：每读到一个 token，就沿 `home` 的 `right` 指针下探到
//! `priority` 不再更大的位置（`:442` 的严格 `<`），把新节点接上去（`:445` 的
//! `node.setLeft(right)`）。它与 Pratt 解析器在**同一张优先级表**下产出**同构的二叉树**
//! （左结合由严格 `<` 保证，见 §13.1 的等价性论证）。本切片选 Pratt 是因为它把
//! "前缀/中缀/后缀"三类位置显式分开，可读、可测；绑定力数值**直接取产品源码的
//! `priority`**，两边不会漂移。
//!
//! # 三条位置规则（都对齐产品）
//!
//! | 位置 | 绑定力 | 依据 |
//! |------|--------|------|
//! | 前缀 `!`/`not`/`&` | 16 | `Not.java:17`、`Address.java:24` |
//! | 后缀 `(`/`[`（`Offset`/`Move`/`DSOffset`） | 17 | `Offset.java:18`、`Move.java:54`、`DSOffset.java:18` |
//! | 前缀 `-`/`+` | 18 | `Negative.java:21`、`Plus.java:18` |
//! | 原子 | 19 | `Node.java:54` 的默认值 |
//!
//! 于是 `not B2[1]` ⇒ `not (B2[1])`（后缀 17 ≥ 16），而 `-B2[1]` ⇒ `(-B2)[1]`（17 < 18）。
//! 这两条**都是从优先级推出来的**，不是约定 —— 见 §13.1。
//!
//! # 为什么 Pratt 与线性右脊下探等价（`Node.java:57` 是关键）
//!
//! 括号里的节点会拿到 **`priority += inBrackets * 20`**（`Node.java:57`）—— 于是
//! `(1+2)*3` 里括号子树的优先级是 33，任何运算符都插不进去，树的形状被锁死。
//! Pratt 用上面的三处绑定力达到同一效果：`(1+2)` 走前缀位置的递归 `parse_bp(0)`，
//! 等价于 `+20` 的作用。
//!
//! ⚠ **一处已知不建模**：函数节点的 `priority` 有三个类改过（`lefthead`/`tophead`=17、
//! `mcorder`=16，见 [`super::names::fun_priority`]），而本切片把**所有调用当原子（19）**。
//! 只有当这些函数与前缀/后缀混写时树形才可能不同，§13.5 已登记为未覆盖。

use crate::error::Result;

use super::ast::{Expr, UnOp};
use super::atom::unary;
use super::classify::{err, infix};
use super::lexer::lex;
use super::token::Tok;

/// 后缀 `(...)`/`[...]` 的绑定力（`SpecialFunction` 子类统一 17）。
const POSTFIX_BP: u8 = 17;
/// 前缀 `!` / `not` / `&`。
const PREFIX_NOT_BP: u8 = 16;
/// 前缀一元 `-` / `+`。
const PREFIX_SIGN_BP: u8 = 18;

/// 解析一条完整表达式（顶层）。要求整串被消费完。
pub fn parse(src: &str) -> Result<Expr> {
    let toks = lex(src)?;
    parse_tokens(&toks)
}

/// 在给定 token 切片上解析一条表达式（参数/列表的每个子段都走这里）。
pub fn parse_tokens(toks: &[Tok]) -> Result<Expr> {
    let mut p = Parser { toks, pos: 0 };
    let e = p.parse_bp(0)?;
    if !matches!(p.peek(), Tok::Eof) {
        return Err(err(format!(
            "表达式在 {} 处无法继续解析（不支持的写法）",
            p.peek().label()
        )));
    }
    Ok(e)
}

/// Pratt 状态机：只带 token 切片与游标，**语义全在 [`super::atom`]**。
pub(super) struct Parser<'a> {
    pub(super) toks: &'a [Tok],
    pub(super) pos: usize,
}

impl<'a> Parser<'a> {
    pub(super) fn peek(&self) -> &Tok {
        self.toks.get(self.pos).unwrap_or(&Tok::Eof)
    }

    pub(super) fn bump(&mut self) {
        if self.pos < self.toks.len() {
            self.pos += 1;
        }
    }

    /// Pratt 主循环。`min_bp` 是当前层要求的最小绑定力。
    fn parse_bp(&mut self, min_bp: u8) -> Result<Expr> {
        let mut lhs = self.parse_prefix()?;
        loop {
            match self.peek() {
                Tok::LParen if POSTFIX_BP >= min_bp => {
                    self.bump();
                    self.take_group(Tok::RParen)?; // Offset：参数对 is_extend 无用，丢弃
                    lhs = Expr::Index { target: Box::new(lhs) };
                }
                Tok::LBracket if POSTFIX_BP >= min_bp => {
                    self.bump();
                    self.take_group(Tok::RBracket)?; // Move / DSOffset
                    lhs = Expr::Index { target: Box::new(lhs) };
                }
                t => {
                    let Some((op, bp)) = infix(t) else { break };
                    if bp < min_bp {
                        break;
                    }
                    self.bump();
                    // 左结合 ⇒ 右操作数用 bp+1 绑定力
                    let rhs = self.parse_bp(bp + 1)?;
                    lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
                }
            }
        }
        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr> {
        let t = self.peek().clone();
        match t {
            Tok::Bang => {
                self.bump();
                Ok(unary(UnOp::Not, self.parse_bp(PREFIX_NOT_BP)?))
            }
            Tok::Amp => {
                self.bump();
                Ok(unary(UnOp::Address, self.parse_bp(PREFIX_NOT_BP)?))
            }
            Tok::Minus => {
                self.bump();
                Ok(unary(UnOp::Neg, self.parse_bp(PREFIX_SIGN_BP)?))
            }
            Tok::Plus => {
                self.bump();
                Ok(unary(UnOp::Pos, self.parse_bp(PREFIX_SIGN_BP)?))
            }
            // `not` 是关键字式前缀运算符（Expression.java:163-165）
            Tok::Word(ref w) if w.eq_ignore_ascii_case("not") => {
                self.bump();
                Ok(unary(UnOp::Not, self.parse_bp(PREFIX_NOT_BP)?))
            }
            // `(` 在原子位置是**分组**（Expression.java:302-317：preNode 为空或运算符时不起 Offset）
            Tok::LParen => {
                self.bump();
                let e = self.parse_bp(0)?;
                self.expect(Tok::RParen)?;
                Ok(e)
            }
            Tok::LBracket => self.parse_value_list(),
            Tok::LBrace => self.parse_select(),
            Tok::Word(_) | Tok::Str(_) => self.parse_primary(),
            other => Err(err(format!("表达式不能以 {} 开头", other.label()))),
        }
    }

    /// 取到与 `close` 配对的括号，返回**不含括号**的 token 切片，并把游标推进到括号之后。
    pub(super) fn take_group(&mut self, close: Tok) -> Result<&'a [Tok]> {
        let toks: &'a [Tok] = self.toks;
        let start = self.pos;
        let mut depth = 0i32;
        let mut end = None;
        let mut i = self.pos;
        while i < toks.len() {
            match toks[i].clone() {
                Tok::Eof => break,
                Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBracket | Tok::RBrace => {
                    if depth == 0 {
                        if toks[i] == close {
                            end = Some(i);
                        } else {
                            return Err(err(format!(
                                "括号不匹配：期望 {}，遇到 {}",
                                close.label(),
                                toks[i].label()
                            )));
                        }
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            i += 1;
        }
        match end {
            Some(e) => {
                self.pos = e + 1;
                Ok(&toks[start..e])
            }
            None => Err(err(format!("括号不匹配：缺少 {}", close.label()))),
        }
    }

    pub(super) fn expect(&mut self, want: Tok) -> Result<()> {
        if self.peek() == &want {
            self.bump();
            Ok(())
        } else {
            Err(err(format!(
                "期望 {}，实际是 {}",
                want.label(),
                self.peek().label()
            )))
        }
    }
}
