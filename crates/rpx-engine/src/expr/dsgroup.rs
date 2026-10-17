//! 分组实例保存原始行号，附属格自动在组内取数。
#[cfg(test)]
mod tests;

use super::{
    ast::Call,
    ds::DsCur,
    dsselect::{decode::decode, eval_bound, rows::multi_key_sort},
    eval::EvalCtx,
    num::{as_bool, val_eq},
};
use crate::{
    error::{EngineError, Result},
    value::Value,
};

pub(crate) fn group(c: &Call, dsn: &str, ctx: &mut dyn EvalCtx) -> Result<Value> {
    let args = decode(c, ctx, 2)?;
    let force_root = c.option.as_deref().unwrap_or("").contains('r');
    let (saved, mut rows) = {
        let set = ctx
            .ds(dsn)
            .ok_or_else(|| EngineError::MissingDataset { name: dsn.into() })?;
        (
            set.current,
            if force_root {
                (0..set.rows.len()).collect()
            } else {
                set.current_rows()
            },
        )
    };
    let result = (|| {
        // 先过滤后排序：被排除的 null/非法日期不能参与键求值。
        if let Some(filter) = &args.flt_exp {
            let mut kept = Vec::new();
            for row in rows {
                ctx.ds(dsn)
                    .ok_or_else(|| missing(dsn))?
                    .set_current(DsCur::Row(row));
                if as_bool(&eval_bound(filter, dsn, ctx)?)? {
                    kept.push(row);
                }
            }
            rows = kept;
        }
        if !args.sort_exps.is_empty() {
            rows = multi_key_sort(&rows, &args.sort_exps, &args.is_descs, dsn, ctx)?;
        } else if args.order != 0 {
            rows = multi_key_sort(
                &rows,
                std::slice::from_ref(&args.select_exp),
                &[args.order < 0],
                dsn,
                ctx,
            )?;
        }
        let mut keys = Vec::<Value>::new();
        let mut members = Vec::<Vec<usize>>::new();
        for row in rows {
            ctx.ds(dsn)
                .ok_or_else(|| missing(dsn))?
                .set_current(DsCur::Row(row));
            let key = eval_bound(&args.select_exp, dsn, ctx)?;
            let index = match keys.iter().position(|v| val_eq(v, &key)) {
                Some(i) => i,
                None => {
                    keys.push(key);
                    members.push(Vec::new());
                    keys.len() - 1
                }
            };
            members[index].push(row);
        }
        let set = ctx.ds(dsn).ok_or_else(|| missing(dsn))?;
        let bindings: Vec<_> = members
            .into_iter()
            .map(|rows| DsCur::Group(set.intern_group(rows)))
            .collect();
        if let Some(ds) = ctx.ds_index(dsn) {
            ctx.note_ds_members(ds, &bindings);
        }
        Ok(Value::List(keys))
    })();
    ctx.ds(dsn).ok_or_else(|| missing(dsn))?.set_current(saved);
    result
}

fn missing(dsn: &str) -> EngineError {
    EngineError::MissingDataset { name: dsn.into() }
}
