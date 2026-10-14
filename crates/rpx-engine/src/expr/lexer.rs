//! 表达式词法分析 —— 对齐产品 `Expression.scanId`（`Expression.java:70-78`）的扫描规则。
//!
//! 算法正文见 `docs/m2-engine-design.md` §13.1（token 集与扫描优先级）。
//! token 类型本身在 [`super::token`]（拆分理由见那里的模块文档）。
//!
//! # 扫描规则（逐条对齐 scanId，**不是**"随便一个 tokenizer"）
//!
//! `scanId` 是产品里唯一的"取一个原子"函数：它从当前位置**一直吃到分隔符为止**，
//! 分隔符集合是（`Expression.java:74`，逐字）：
//!
//! ```text
//! 空白 + - * / % = & | ! : , > < ( ) [ ] { }
//! ```
//!
//! ⛔ **`.` 不在分隔符集合里** —— 所以 `ds1.select` 会被整体吃成一个原子（不是三个），
//! 这正是 `Expression.java:174` 之所以能用 `id.indexOf('.')` 切出数据集名的前提。
//! 逐字判定见 [`is_delimiter`]。
//!
//! # 本切片的三处**刻意偏差**（都写在这里，别当结论外推）
//!
//! 1. `;` 在本模块升级为独立 token（`Tok::Semi`）。产品里 `;` **从不**出现在一个
//!    Expression 内部 —— 它是 `ParamParser` 的**参数级**分隔符（`ParamParser.java:66-79`
//!    的 `'\0' → ';' → ',' → ':'` 层级），参数串在切分完之后才逐段交给 Expression 解析。
//!    本切片把参数切分与表达式解析放在同一趟，所以必须让 `;` 可见。
//! 2. 字符串反转义 = `Escape.remove`（`Escape.java:173-214`，已**逐行核对**）。
//!    真值表（escapeChar = `\`）：`\t`→9、`\r`→13、`\n`→10、
//!    **其它 `\x` 只留 `x`（反斜杠被丢掉）**、非 escape 字符原样；
//!    ⛔ **结尾的孤立 `\` 被丢弃**（`Escape.java:188` 的 `if (++i == len) break;`）。
//!    这一处**不影响 `is_extend()`**（谓词只看树形，不看字面量内容），但影响 M2-13 的求值。
//! 3. `$[...]` 的内容也走同一个 [`unescape`]（产品：`Escape.remove(内文.trim())`，
//!    `Expression.java:171`）。⛔ 产品的 `trim()` 在**反转义之前**，本切片顺序相同。
//!
//! # 未支持的写法（诚实清单，见 §13.5）
//!
//! - `|`（单竖线）：产品直接抛 `ReportError`（`Expression.java:397-406`）；
//!   这里产出 `Tok::Pipe` 交给语法层报"不支持"。
//! - 十六进制/日期字面量的**分类**在语法层做（本层只切原子，不判类型）。

use crate::error::{EngineError, Result};

use super::token::{is_delimiter, Tok};

fn err(detail: &str) -> EngineError {
    EngineError::UnsupportedExpr {
        expr: detail.to_string(),
    }
}

/// 词法分析入口。返回的 token 流**总是**以 [`Tok::Eof`] 结尾。
pub fn lex(src: &str) -> Result<Vec<Tok>> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '"' | '\'' => {
                let (text, next) = scan_quoted(&chars, i)?;
                out.push(Tok::Str(text));
                i = next;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            '[' => {
                out.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                out.push(Tok::RBracket);
                i += 1;
            }
            '{' => {
                out.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                out.push(Tok::RBrace);
                i += 1;
            }
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                out.push(Tok::Minus);
                i += 1;
            }
            '*' => {
                out.push(Tok::Star);
                i += 1;
            }
            '/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            '%' => {
                out.push(Tok::Percent);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            ':' => {
                out.push(Tok::Colon);
                i += 1;
            }
            ';' => {
                out.push(Tok::Semi);
                i += 1;
            }
            '=' => {
                let (tok, next) = pair(&chars, i, '=', Tok::EqEq, Tok::Eq);
                out.push(tok);
                i = next;
            }
            '!' => {
                let (tok, next) = pair(&chars, i, '=', Tok::BangEq, Tok::Bang);
                out.push(tok);
                i = next;
            }
            '>' => {
                let (tok, next) = pair(&chars, i, '=', Tok::Ge, Tok::Gt);
                out.push(tok);
                i = next;
            }
            '<' => {
                let (tok, next) = pair(&chars, i, '=', Tok::Le, Tok::Lt);
                out.push(tok);
                i = next;
            }
            '&' => {
                let (tok, next) = pair(&chars, i, '&', Tok::AmpAmp, Tok::Amp);
                out.push(tok);
                i = next;
            }
            '|' => {
                let (tok, next) = pair(&chars, i, '|', Tok::PipePipe, Tok::Pipe);
                out.push(tok);
                i = next;
            }
            _ => {
                // scanId：一直吃到分隔符（`.` 不是分隔符）
                let start = i;
                while i < chars.len() && !is_delimiter(chars[i]) {
                    i += 1;
                }
                out.push(Tok::Word(chars[start..i].iter().collect()));
            }
        }
    }
    out.push(Tok::Eof);
    Ok(out)
}

/// 双字符运算符：若 `chars[i+1] == second` 则吞两个字符，否则吞一个。
fn pair(chars: &[char], i: usize, second: char, two: Tok, one: Tok) -> (Tok, usize) {
    if chars.get(i + 1) == Some(&second) {
        (two, i + 2)
    } else {
        (one, i + 1)
    }
}

/// 扫一个字符串字面量。返回（内容，下一个位置）。`\` 转义标 ⚠ 未逐行核（见模块文档）。
fn scan_quoted(chars: &[char], start: usize) -> Result<(String, usize)> {
    let quote = chars[start];
    let mut i = start + 1;
    let mut raw = String::new();
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            if i + 1 >= chars.len() {
                break;
            }
            raw.push(c);
            raw.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == quote {
            return Ok((unescape(&raw), i + 1));
        }
        raw.push(c);
        i += 1;
    }
    Err(err("未闭合的字符串字面量"))
}

/// `Escape.remove(raw, '\')` 的逐行移植（`Escape.java:173-214`）。
///
/// | 输入 | 输出 | 依据 |
/// |------|------|------|
/// | `\t` / `\r` / `\n` | 9 / 13 / 10 | `Escape.java:191-202` |
/// | 其它 `\x` | 只留 `x`（**反斜杠被丢掉**） | `:203-206` 的 `default` |
/// | 非 `\` 字符 | 原样 | `:208-210` |
/// | **结尾的孤立 `\`** | **整段丢弃**（不留反斜杠） | `:188` 的 `if (++i == len) break;` |
///
/// ⛔ 末行最容易写错：直觉会写成"把孤立反斜杠保留下来"，产品是**丢弃**。
pub fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut it = raw.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            // Escape.java:203-206 default：未知转义只保留后一个字符，反斜杠本身丢弃
            Some(other) => out.push(other),
            // Escape.java:188 尾部孤立 escape ⇒ 产品直接 break（丢弃），不是保留反斜杠
            None => {}
        }
    }
    out
}
