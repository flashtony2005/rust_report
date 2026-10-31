//! 坐标的**语法**：`[...]` 里的 token 片段 → [`Coord`]。
//!
//! 从 `coord.rs` 拆出只为门禁行数（G4：每 `.rs` ≤ 500 行，450 起预警）。
//! 语义正文在 [`super`] 的模块文档 —— 这里只做形状识别，**不碰 head 图**。
//!
//! 记号事实源：润乾官方教程（`056_bzbb121` 说明 a–d、`057_tqb124` 说明 1–5、
//! `055_ljbb117` 的 `E2[-1]`、`061_pm137` 的 `` B2[`0] ``）。
//!
//! # 序号 vs 位移：靠**树形**判，不靠字面量
//!
//! `C2[1]` 的 `1` 是 `Constant` ⇒ 绝对序号；`C2[-1]` 的 `-1` 是 `Negative`
//! 一元节点 ⇒ 位移。这与产品的 `Move` 分派一致（`Expression.java:136-155`），
//! 也比"看字符串有没有 `-`"稳（`C2[1-2]` 这类复合式不会被误判成位移）。

use crate::error::{EngineError, Result};
use crate::expr::ast::{BinOp, Expr, UnOp};
use crate::expr::classify::split_top;
use crate::expr::coord::{is_root_marker, Coord, CoordSeg, CoordVal};
use crate::expr::token::Tok;
use crate::params::ParamTable;
use crate::value::Value;

/// 把 `[...]` 里的 token 片段解析成 [`Coord`]。
///
/// `;` 切纵展/横伸两链（`056` 说明 b），`,` 切链内各段（从远到近）。
pub fn parse_coord(inner: &[Tok]) -> Result<Coord> {
    let mut parts = split_top(inner, &Tok::Semi);
    let has_semi = parts.len() > 1;
    let left = parse_segs(parts.remove(0))?;
    let top = if has_semi {
        parse_segs(parts.remove(0))?
    } else {
        Vec::new()
    };
    Ok(Coord {
        left,
        top,
        has_semi,
    })
}

/// 切一条链（按顶层 `,`）并逐段解码；空段跳过（`[;T2:1]` 的分号前是空段）。
fn parse_segs(toks: &[Tok]) -> Result<Vec<CoordSeg>> {
    let mut out = Vec::new();
    for piece in split_top(toks, &Tok::Comma) {
        if piece.is_empty() {
            continue;
        }
        out.push(decode_seg(piece)?);
    }
    Ok(out)
}

/// 解码一段：`` `0 `` / `A2` / `A2:1` / `A2:+1` / `1` / `-1`。
fn decode_seg(toks: &[Tok]) -> Result<CoordSeg> {
    if let [Tok::Word(w)] = toks {
        if is_root_marker(w) {
            return Ok(CoordSeg {
                master: None,
                value: CoordVal::Root,
            });
        }
    }
    let e = crate::expr::parser::parse_tokens_with(toks, &ParamTable::EMPTY)?;
    match e {
        // `A2` —— 主格，无序号 ⇒ 当前格所属的那个实例
        Expr::CellRef { id, .. } => Ok(CoordSeg {
            master: Some(id),
            value: CoordVal::Current,
        }),
        // `1` —— 无主格的绝对序号；`0` 即"当前格自己"
        Expr::Const(Value::Int(n)) => Ok(CoordSeg {
            master: None,
            value: if n == 0 {
                CoordVal::Current
            } else {
                CoordVal::Index(i64::from(n))
            },
        }),
        // `+1` / `-1` —— 无主格的位移（`055_ljbb117` 的 `E2[-1]`）
        Expr::Unary {
            op: op @ (UnOp::Pos | UnOp::Neg),
            rhs,
        } => Ok(CoordSeg {
            master: None,
            value: CoordVal::Offset(signed_literal(&rhs, op)?),
        }),
        // `A2:1` / `A2:+1` / `A2:-1` —— 主格 + 序号/偏移
        Expr::Binary {
            op: BinOp::Link,
            lhs,
            rhs,
        } => {
            let master = match &*lhs {
                Expr::CellRef { id, .. } => id.clone(),
                other => {
                    return Err(err(format!("坐标段的主格必须是格号，实得 {other:?}")));
                }
            };
            let value = decode_val(&rhs)?;
            Ok(CoordSeg {
                master: Some(master),
                value,
            })
        }
        other => Err(err(format!("坐标段 `{other:?}` 无法识别"))),
    }
}

/// `主格:` 右边那一半：`1` / `0` / `+1` / `-1` / 另一个格号。
fn decode_val(rhs: &Expr) -> Result<CoordVal> {
    match rhs {
        // `A2:B2` —— 产品接受"主格:格号"，此处按"当前格所属的该实例"处理
        Expr::CellRef { .. } => Ok(CoordVal::Current),
        Expr::Const(Value::Int(n)) => Ok(if *n == 0 {
            CoordVal::Current
        } else {
            CoordVal::Index(i64::from(*n))
        }),
        Expr::Unary { op, rhs } => Ok(CoordVal::Offset(signed_literal(rhs, *op)?)),
        other => Err(err(format!(
            "坐标段的值不是整数序号或 ±位移，实得 {other:?}"
        ))),
    }
}

/// 一元 `+`/`-` 的操作数必须是非负整数字面量；返回带符号的值。
fn signed_literal(rhs: &Expr, op: UnOp) -> Result<i64> {
    let Expr::Const(Value::Int(n)) = *rhs else {
        return Err(err(format!(
            "位移坐标的偏移量必须是整数字面量，实得 {rhs:?}"
        )));
    };
    let n = i64::from(n);
    Ok(if op == UnOp::Neg { -n } else { n })
}

fn err(detail: String) -> EngineError {
    EngineError::UnsupportedExpr { expr: detail }
}
