//! 教程排名、补空行、条件分页和同源编号所需的普通函数。
use super::{evaluate, EvalCtx};
use crate::error::{EngineError, Result};
use crate::expr::ast::{Call, Expr};
use crate::expr::num::{as_bool, to_f64};
use crate::value::Value;

fn invalid(c: &Call) -> EngineError {
    EngineError::UnsupportedExpr {
        expr: format!("{}：参数数量或类型不正确", c.name),
    }
}

fn integer(v: &Value, c: &Call) -> Result<i32> {
    let n = match v {
        Value::Text(s) => s.parse::<f64>().ok(),
        _ => to_f64(v),
    }
    .ok_or_else(|| invalid(c))?;
    if !n.is_finite() || n < i32::MIN as f64 || n > i32::MAX as f64 {
        return Err(invalid(c));
    }
    Ok(n as i32)
}

pub(super) fn call(c: &Call, ctx: &mut dyn EvalCtx) -> Result<Value> {
    match (c.name.as_str(), c.args.as_slice()) {
        ("if", [condition, yes, no]) => {
            let branch = if as_bool(&evaluate(condition, ctx)?)? {
                yes
            } else {
                no
            };
            evaluate(branch, ctx)
        }
        ("ifn", [value, default]) => {
            let value = evaluate(value, ctx)?;
            if value.is_null() {
                evaluate(default, ctx)
            } else {
                Ok(value)
            }
        }
        ("int", [value]) => Ok(Value::Int(integer(&evaluate(value, ctx)?, c)?)),
        ("value", []) => Ok(ctx.cell_value(ctx.current())),
        ("count", [value]) => {
            let n = match evaluate(value, ctx)? {
                Value::List(values) => values.len(),
                Value::Null => 0,
                _ => 1,
            };
            Ok(Value::Int(i32::try_from(n).map_err(|_| invalid(c))?))
        }
        ("to", [begin, end]) => {
            let begin = integer(&evaluate(begin, ctx)?, c)?;
            let end = integer(&evaluate(end, ctx)?, c)?;
            // A bounded list is required before the extension engine allocates rows.
            let len = (i64::from(end) - i64::from(begin) + 1).max(0);
            if len > 1_048_576 {
                return Err(invalid(c));
            }
            Ok(Value::List((begin..=end).map(Value::Int).collect()))
        }
        ("seq", [Expr::CellRef { id, .. }]) => {
            let src = crate::expr::cellset::target_source(id, ctx)?;
            let instance =
                crate::expr::resolve::resolve_cell(ctx, id, false).ok_or_else(|| invalid(c))?;
            let all = crate::expr::coord::instances_of(src, ctx);
            let index = all
                .iter()
                .position(|&v| v == instance)
                .ok_or_else(|| invalid(c))?;
            Ok(Value::Int(index as i32 + 1))
        }
        _ => Err(invalid(c)),
    }
}
