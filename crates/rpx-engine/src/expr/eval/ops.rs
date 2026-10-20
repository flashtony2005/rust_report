//! 一元 / 二元运算的求值 —— [`super::evaluate`] 的两个叶子分派。
//!
//! # 为什么从 `eval.rs` 拆出来
//!
//! 只有**门禁行数**一个理由（G4：每 `.rs` ≤ 500 行，450 起预警）。`eval.rs` 在
//! 坐标四原语落地后涨到 527 行（HEAD 已是 491，只余 9 行余量）⇒ 把最内聚、最
//! 不依赖上下文的一块（纯 `Expr → Value` 的运算符分派）搬出来。
//! 拆法与 `expr/coord.rs` + `expr/coord/parse.rs` 同款：`foo.rs` 旁边放 `foo/`。
//!
//! # `&Cellx` 的求值时机（本模块唯一一处非平凡语义）
//!
//! [`evaluate_unary`] 对 `UnOp::Address` **提前返回、不求值 `rhs`** —— 它要的是
//! `rhs` 的**格号**而不是格值（教程 `063_xh141`「Cellx 必须是当前格的主格」）。
//! 旧实现先 `evaluate(rhs)` 再报不支持，等于把"算错了"和"不支持"混成同一句话。

use crate::error::{EngineError, Result};
use crate::expr::ast::{BinOp, Expr, UnOp};
use crate::value::Value;

use super::super::coord;
use super::super::num::{
    as_bool, num_add, num_cmp, num_div, num_mod, num_mul, num_neg, num_sub, val_eq,
};
use super::{evaluate, EvalCtx};

/// 一元运算（`Expression.java` 的 `preNode` 分派）。
pub(super) fn evaluate_unary(op: UnOp, rhs: &Expr, ctx: &mut dyn EvalCtx) -> Result<Value> {
    // ⛔ `&Cellx` **不能先求值 rhs**：它要的是 rhs 的**格号**，不是格值
    //    （产品 `Address.java`；教程 `063_xh141`「Cellx 必须是当前格的主格」）。
    if op == UnOp::Address {
        let Expr::CellRef { id, .. } = rhs else {
            return Err(EngineError::UnsupportedExpr {
                expr: format!("& 的操作数必须是格号，实得 {rhs:?}"),
            });
        };
        return coord::address_of(id, ctx);
    }
    let v = scalar(evaluate(rhs, ctx)?);
    match op {
        UnOp::Neg => num_neg(&v),
        UnOp::Pos => Ok(v),
        UnOp::Not => Ok(Value::Bool(!as_bool(&v)?)),
        // 上面已提前返回
        UnOp::Address => unreachable!("Address 在求值 rhs 之前已处理"),
    }
}

/// 标量语境降级：产品 `Variant2.getValue` 对 CSVariable/List 取**第一个**值，
/// 空集取 null（教程「定位到多个单元格 ⇒ 返回第一个」）。
///
/// 只用于一元/二元运算符的操作数；函数实参**不降级**（`sum(B2[`0]{})` 要整集）。
fn scalar(v: Value) -> Value {
    match v {
        Value::List(items) => items.into_iter().next().unwrap_or(Value::Null),
        other => other,
    }
}

/// 二元运算。`Link`/`Comma`/`Eval` **不走这里** —— 它们是参数分隔符或
/// 复合参数形态，由调用方（`cellset` / `Call` 的 `param_kind`）另行处理。
pub(super) fn evaluate_binary(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    ctx: &mut dyn EvalCtx,
) -> Result<Value> {
    // M5-09：坐标/格集作为操作数时先降级成标量（空集 ⇒ null）。
    // `in` 的右操作数必须保留 List（`x in [1,2]`），故整条支路不降级。
    let (a0, b0) = (evaluate(lhs, ctx)?, evaluate(rhs, ctx)?);
    let (a, b) = if op == BinOp::In {
        (a0, b0)
    } else {
        (scalar(a0), scalar(b0))
    };
    match op {
        BinOp::Add => num_add(&a, &b),
        BinOp::Sub => num_sub(&a, &b),
        BinOp::Mul => num_mul(&a, &b),
        BinOp::Div => num_div(&a, &b),
        BinOp::Mod => num_mod(&a, &b),
        BinOp::Eq => Ok(Value::Bool(val_eq(&a, &b))),
        BinOp::Ne => Ok(Value::Bool(!val_eq(&a, &b))),
        BinOp::In => {
            let contains = |value: &Value| match &b {
                Value::List(items) => items.iter().any(|item| val_eq(value, item)),
                _ => val_eq(value, &b),
            };
            Ok(Value::Bool(match &a {
                Value::List(items) => items.iter().all(contains),
                _ => contains(&a),
            }))
        }
        BinOp::Gt => Ok(Value::Bool(num_cmp(&a, &b)? == std::cmp::Ordering::Greater)),
        BinOp::Ge => Ok(Value::Bool(num_cmp(&a, &b)? != std::cmp::Ordering::Less)),
        BinOp::Lt => Ok(Value::Bool(num_cmp(&a, &b)? == std::cmp::Ordering::Less)),
        BinOp::Le => Ok(Value::Bool(num_cmp(&a, &b)? != std::cmp::Ordering::Greater)),
        BinOp::And => Ok(Value::Bool(as_bool(&a)? && as_bool(&b)?)),
        BinOp::Or => Ok(Value::Bool(as_bool(&a)? || as_bool(&b)?)),
        // Link 单独出现（`A:B` 不在 sum 里）——产品语义是"复合参数"，无独立求值面
        BinOp::Link | BinOp::Comma | BinOp::Eval => Err(EngineError::UnsupportedExpr {
            expr: format!("{op:?}"),
        }),
    }
}
