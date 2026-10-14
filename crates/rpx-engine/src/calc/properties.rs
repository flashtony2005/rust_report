//! 行/列轴与普通格的 `calcProperties`。
//!
//! Java 源码：`ExtCellSet.java:1188-1221`、`ExtRowCell.java:346-459`、
//! `ExtColCell.java:313-407`、`ExtNormalCell.java:703-799`。
//! 属性 AST 保存在每个格的 `CellRuntime` 中，扩展克隆继承 AST，求值覆盖值各自保存。

use crate::arena::CellId;
use crate::calc::ctx::CalcCtx;
use crate::calc::ds_current::set_ds_current_for_cell;
use crate::error::{EngineError, Result};
use crate::eval::sched::{run_channels, SchedGrid};
use crate::eval::state;
use crate::expr::eval::{evaluate, EvalCtx};
use crate::value::{single_value, Value};

/// 按 Java `finish=false` 顺序执行属性 pass：行轴 → 列轴 → 普通格。
pub(crate) fn calculate_properties(ctx: &mut CalcCtx) -> Result<()> {
    let rows = ctx.grid.row_size;
    let cols = ctx.grid.col_size;

    for row in 1..rows {
        let Some(&owner) = ctx.grid.by_pos.get(&(row, 0)) else {
            continue;
        };
        // `ExtRowCell.calcProperties` 把本行首个非空主体格设为 current。
        let current = (1..cols)
            .find_map(|col| ctx.grid.by_pos.get(&(row, col)).copied())
            .unwrap_or(owner);
        calculate_target(ctx, owner, current, PropertyKind::Row)?;
    }

    for col in 1..cols {
        let Some(&owner) = ctx.grid.by_pos.get(&(0, col)) else {
            continue;
        };
        calculate_target(ctx, owner, owner, PropertyKind::Column)?;
    }

    // `ExtCellSet.calcProperties` 只扫描主格，按行优先顺序。
    for row in 1..rows {
        for col in 1..cols {
            let Some(&owner) = ctx.grid.by_pos.get(&(row, col)) else {
                continue;
            };
            let primary = ctx
                .grid
                .get(owner)
                .is_some_and(|cell| cell.row == row && cell.col == col);
            if primary {
                calculate_target(ctx, owner, owner, PropertyKind::Cell)?;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PropertyKind {
    Row,
    Column,
    Cell,
}

fn calculate_target(
    ctx: &mut CalcCtx,
    owner: CellId,
    current: CellId,
    kind: PropertyKind,
) -> Result<()> {
    let expressions = ctx
        .grid
        .get(owner)
        .map(|cell| cell.rt.property_exprs.clone())
        .unwrap_or_default();
    if expressions.is_empty() {
        return Ok(());
    }

    if matches!(kind, PropertyKind::Cell) {
        // `NormalCell.calcProperties` 设置 current 与 head 对跟踪器；只有 head 对变化时才刷新 DS current。
        ctx.prepare_eval(owner);
        if ctx.tracker.is_changed() {
            set_ds_current_for_cell(ctx, owner)?;
        }
    } else {
        // 行/列头的 `calcProperties` 设置 current，但不设置 head 对跟踪器。
        ctx.set_current(current);
    }

    for (key, expr) in expressions {
        let value = evaluate_property(ctx, owner, current, kind, &expr)?;
        let value = match kind {
            PropertyKind::Cell => single_value(&value).cloned().unwrap_or(Value::Null),
            PropertyKind::Row | PropertyKind::Column => builtin_value(key, value)?,
        };
        store_property(ctx, owner, key, value);
    }
    Ok(())
}

/// 求值一个属性 AST。格值依赖产生的 Suspend 交给与普通值计算相同的
/// channel4/5/6 trampoline；随后恢复属性 current 并重试 AST。
fn evaluate_property(
    ctx: &mut CalcCtx,
    owner: CellId,
    current: CellId,
    kind: PropertyKind,
    expr: &crate::expr::Expr,
) -> Result<Value> {
    if expr.get_state() > ctx.get_state() && expr.get_state() == state::THIRD {
        return Err(EngineError::UnsupportedExpr {
            expr: format!(
                "属性表达式 state 8 暂不在当前表达式子集中（cell #{})",
                owner.0
            ),
        });
    }

    match evaluate(expr, ctx) {
        Ok(value) => Ok(value),
        Err(EngineError::Suspend(suspend))
            if matches!(suspend.state, state::FIRST | state::SECOND | state::SPECIAL) =>
        {
            // 依赖链及其 `Suspend{cell,state}` 转移均由同一调度器处理；不递归求值表达式。
            run_channels(ctx)?;
            if matches!(kind, PropertyKind::Cell) {
                ctx.prepare_eval(owner);
                if ctx.tracker.is_changed() {
                    set_ds_current_for_cell(ctx, owner)?;
                }
            } else {
                ctx.set_current(current);
            }
            match evaluate(expr, ctx) {
                Ok(value) => Ok(value),
                Err(EngineError::Suspend(next)) => Err(EngineError::UnsupportedExpr {
                    expr: format!(
                        "属性表达式依赖未能由求值通道消化：cell #{} state {}（再次请求 cell #{} state {}）",
                        owner.0, suspend.state, next.cell.0, next.state
                    ),
                }),
                Err(error) => Err(error),
            }
        }
        Err(EngineError::Suspend(suspend)) => Err(EngineError::UnsupportedExpr {
            expr: format!(
                "属性表达式挂起状态 {} 不属于值通道（cell #{})",
                suspend.state, suspend.cell.0
            ),
        }),
        Err(error) => Err(error),
    }
}

/// `Variant2.getValue` 保留 List，Java 的类型 setter 随后执行强制转换。
/// 因此内置行/列属性遇到 List 必须报错，不能取首项；只有普通格属性使用 `getSingleValue`。
fn builtin_value(key: u8, value: Value) -> Result<Value> {
    match key {
        1 | 11 => as_float(value, key), // 行高 / 列宽
        2 | 7 | 12 | 13 | 15 => match value {
            Value::Bool(_) => Ok(value),
            other => Err(property_type_error(key, "boolean", other)),
        },
        _ => Err(EngineError::Invariant {
            detail: format!("unsupported built-in property key {key}"),
        }),
    }
}

fn as_float(value: Value, key: u8) -> Result<Value> {
    let number = match &value {
        Value::Int(v) => *v as f32,
        Value::Long(v) => *v as f32,
        Value::Double(v) => *v as f32,
        Value::Float(v) => *v,
        Value::Short(v) => *v as f32,
        Value::Byte(v) => *v as f32,
        Value::Decimal(v) => v
            .parse::<f32>()
            .map_err(|_| property_type_error(key, "number", value.clone()))?,
        _ => return Err(property_type_error(key, "number", value)),
    };
    Ok(Value::Float(number))
}

fn property_type_error(key: u8, expected: &str, got: Value) -> EngineError {
    EngineError::UnsupportedExpr {
        expr: format!(
            "属性 key {key} 需要 {expected}，实际为 {} ({got})",
            got.type_name()
        ),
    }
}

fn store_property(ctx: &mut CalcCtx, owner: CellId, key: u8, value: Value) {
    if let Some(rt) = ctx.rt_mut(owner) {
        if let Some((_, old)) = rt.property_values.iter_mut().find(|(k, _)| *k == key) {
            *old = value;
        } else {
            rt.property_values.push((key, value));
        }
    }
}

/// 从运行态格读取一个内置属性覆盖值。
pub(crate) fn property_value(ctx: &CalcCtx, owner: CellId, key: u8) -> Option<&Value> {
    ctx.rt(owner)?
        .property_values
        .iter()
        .find(|(property, _)| *property == key)
        .map(|(_, value)| value)
}

/// 取几何属性的浮点覆盖值；计算态几何投影与该转换共用此入口。
pub(crate) fn float_property(ctx: &CalcCtx, owner: CellId, key: u8) -> Option<f32> {
    match property_value(ctx, owner, key)? {
        Value::Float(value) => Some(*value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{as_float, builtin_value, calculate_target, float_property, property_value};
    use crate::value::Value;

    #[test]
    fn builtin_geometry_properties_are_float_and_preserve_boolean_values() {
        assert_eq!(as_float(Value::Int(40), 1).unwrap(), Value::Float(40.0));
        assert_eq!(
            builtin_value(2, Value::Bool(false)).unwrap(),
            Value::Bool(false)
        );
        assert!(builtin_value(1, Value::List(vec![Value::Int(40)])).is_err());
    }

    #[test]
    fn row_and_column_properties_do_not_split_list_values() {
        let err = builtin_value(11, Value::List(vec![Value::Int(25)])).unwrap_err();
        assert!(err.to_string().contains("需要 number"));
    }

    #[test]
    fn row_height_property_is_read_as_a_float_overlay() {
        use std::path::PathBuf;

        use crate::calc::ctx::CalcCtx;
        use crate::stage_b::prepare_calculate;
        use crate::value::Value;

        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let bytes = std::fs::read(fixtures.join("engine_rowheight.rpx")).unwrap();
        let report = rpx_core::open_bytes(&bytes).unwrap();
        let datasets = crate::dataset::load_datasets(&fixtures, &report).unwrap();
        let stage = prepare_calculate(&report).unwrap();
        let mut ctx = CalcCtx::new(stage.grid, datasets);
        let row = *ctx.grid.by_pos.get(&(2, 0)).unwrap();
        ctx.rt_mut(row)
            .unwrap()
            .property_values
            .push((1, Value::Float(40.0)));

        assert_eq!(float_property(&ctx, row, 1), Some(40.0));
    }

    #[test]
    fn a_property_cell_reference_suspend_is_resumed_by_the_value_trampoline() {
        use std::path::PathBuf;

        use crate::arena::CellId;
        use crate::calc::ctx::CalcCtx;
        use crate::stage_b::prepare_calculate;
        use crate::value::Value;

        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let report_bytes = std::fs::read(fixtures.join("engine_rowheight.rpx")).unwrap();
        let report = rpx_core::open_bytes(&report_bytes).unwrap();
        let datasets = crate::dataset::load_datasets(&fixtures, &report).unwrap();
        let stage = prepare_calculate(&report).unwrap();
        let mut ctx = CalcCtx::new(stage.grid, datasets);

        let row_owner = *ctx.grid.by_pos.get(&(2, 0)).unwrap();
        let current = *ctx.grid.by_pos.get(&(2, 1)).unwrap();
        let dependency = *ctx.grid.by_pos.get(&(2, 3)).unwrap();
        ctx.grid.cells[row_owner.0 as usize].rt.property_exprs =
            vec![(1, crate::expr::parse("C2").unwrap())];
        let dep = &mut ctx.grid.cells[dependency.0 as usize];
        dep.rt.calculated = false;
        dep.rt.demanded = false;
        dep.value = Value::Null;

        calculate_target(&mut ctx, row_owner, current, super::PropertyKind::Row).unwrap();

        assert_eq!(
            property_value(&ctx, row_owner, 1),
            Some(&Value::Float(20.0))
        );
        assert_eq!(ctx.value_of(CellId(dependency.0)), Value::Int(20));
    }
}
