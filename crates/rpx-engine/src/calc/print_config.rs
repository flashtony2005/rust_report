use super::{ctx::CalcCtx, ds_current::set_ds_current_for_cell};
use crate::{
    arena::{CalcGrid, CellId},
    error::{EngineError, Result},
    expr::{eval::EvalCtx, evaluate, num::as_bool, parse_with},
    macros::{substitute, MacroTable},
    params::ParamTable,
    value::Value,
};
use rpx_core::ReportDefine;

pub(super) fn apply(
    ctx: &mut CalcCtx,
    rd: &ReportDefine,
    out: &mut CalcGrid,
    params: &ParamTable,
    macros: &MacroTable,
) -> Result<()> {
    out.print_setup = rd.print_setup.clone();
    out.print_options = crate::pager::options::PrintOptions::from_report(rd)?;
    for r in 1..rd.rows {
        for c in 1..rd.cols {
            if let Some(cell) = rd.cell_at(r, c) {
                out.print_styles.insert((r as i32, c as i32), cell.into());
            }
        }
    }
    for (key, value) in &rd.custom_properties {
        let Some(text) = value.as_deref() else {
            continue;
        };
        if key == "rust_report.print.rows" {
            ctx.set_current(CellId::ROOT);
            set_ds_current_for_cell(ctx, CellId::ROOT)?;
            out.print_setup.row_num_per_page = match eval(text, ctx, params, macros)? {
                Value::Int(n) if n > 0 => n,
                _ => return Err(invalid("每页行数表达式必须返回正整数")),
            };
        } else if let Some(address) = key.strip_prefix("rust_report.print.group.") {
            let source = ctx
                .source_by_id(address)
                .ok_or_else(|| invalid("组后分页引用不存在的格"))?;
            if !ctx
                .grid
                .get(source)
                .is_some_and(|h| h.is_extender && h.extend == 18)
            {
                return Err(invalid("组后分页必须引用纵向扩展格"));
            }
            for id in crate::expr::coord::instances_of(source, ctx) {
                ctx.set_current(id);
                ctx.save_current();
                set_ds_current_for_cell(ctx, id)?;
                if as_bool(&eval(text, ctx, params, macros)?)? {
                    let h = ctx.grid.get(id).ok_or_else(|| invalid("组实例丢失"))?;
                    let end = h.row + h.row_merge - 1;
                    if let Some(flag) = out.row_break_page.get_mut((end - 1) as usize) {
                        *flag = true
                    }
                }
            }
        }
    }
    Ok(())
}
fn eval(text: &str, ctx: &mut CalcCtx, params: &ParamTable, macros: &MacroTable) -> Result<Value> {
    evaluate(
        &parse_with(&substitute(text.trim_start_matches('='), macros)?, params)?,
        ctx,
    )
}
fn invalid(detail: &str) -> EngineError {
    EngineError::UnsupportedPaging {
        detail: detail.into(),
    }
}

/// 图形占用真实宿主跨度，按需扩展可见行列，避免连续分组的图形互相覆盖。
pub(super) fn fit_graphics(grid: &mut CalcGrid) {
    if !grid.print_options.auto_size {
        return;
    }
    let pixel_per_unit = crate::pager::unit_to_pt(1.0, grid.unit) as f32 * 96.0 / 72.0;
    for visual in &grid.graphics {
        if matches!(
            visual.content,
            crate::graphics::GraphicContent::CellBackground(_)
        ) {
            continue;
        }
        for (begin, count, size, visible, target) in [
            (
                visual.row,
                visual.row_span,
                visual.height as f32,
                &grid.row_visible,
                &mut grid.row_heights,
            ),
            (
                visual.col,
                visual.col_span,
                visual.width as f32,
                &grid.col_visible,
                &mut grid.col_widths,
            ),
        ] {
            let indices = ((begin - 1) as usize..((begin + count - 1) as usize).min(target.len()))
                .filter(|i| visible.get(*i) != Some(&false))
                .collect::<Vec<_>>();
            let used = indices
                .iter()
                .map(|i| target[*i] * pixel_per_unit)
                .sum::<f32>();
            let extra = (size - used).max(0.0) / pixel_per_unit / indices.len().max(1) as f32;
            for i in indices {
                target[i] += extra
            }
        }
    }
}
