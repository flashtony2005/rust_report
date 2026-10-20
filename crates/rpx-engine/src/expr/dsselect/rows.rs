//! Root 态的行操作：定序 [`eval_rows_idx`]（`Group.evaluateRows:430-453`）
//! 与 `;` 多键排序 [`multi_key_sort`]（`Group.sort` → `SortDsValue.sort:343-414`）。
//!
//! 两者都按「保存指针 → 逐行 setCurrent → 求值 → 恢复」的产品循环形态
//! （`Row.java:97-110` 的 save/setCurrent/finally 展开）。

use std::cmp::Ordering;

use crate::error::Result;
use crate::expr::ast::Expr;
use crate::expr::ds::DsCur;
use crate::expr::eval::EvalCtx;
use crate::expr::num::aggregate_cmp;
use crate::value::Value;

use super::{eval_bound, missing};

/// `Group.evaluateRows`（`Group.java:430-453`）的定序版：按 `idx` 次序逐行
/// 把指针切到该行求值，返回**同序**的值列表；顺带写 `selected` 缓存
/// （产品 `row.setValue(...)` 的落点）。
pub(super) fn eval_rows_idx(
    dsn: &str,
    idx: &[usize],
    col_expr: &Expr,
    ctx: &mut dyn EvalCtx,
) -> Result<Vec<Value>> {
    let saved = {
        let set = ctx.ds(dsn).ok_or_else(|| missing(dsn))?;
        set.current
    };
    let result = (|| {
        let mut out = Vec::with_capacity(idx.len());
        for &i in idx {
            ctx.ds(dsn).expect("上面刚取到").set_current(DsCur::Row(i));
            let v = eval_bound(col_expr, dsn, ctx)?;
            ctx.ds(dsn).expect("同上").selected[i] = Some(v.clone());
            out.push(v);
        }
        Ok(out)
    })();
    ctx.ds(dsn).expect("同上").set_current(saved);
    result
}

/// `;` 多键排序：`Group.sort(sortExps, isDescs)` → `SortDsValue.sort:343-414`。
///
/// 产品算法 = 键0 排序 + 相等 run 内递归按键1…（`:384-401` 的 run 切分）。
/// Rust 用**稳定多键比较**（`sort_by` 稳定）等价 —— 两趟都稳定的手写归并
/// 在全键相等时产出 = 输入序，Rust 稳定排序同结论。
///
/// 2026-10-20 已探针实证并夹具化（`ProbeM5Rest` P1 + `engine_tiesort`）：
/// 全键相等 7/3/9/1/5 原样输出 ⇒ `spec-m5.md` §6.3 ④ 闭合，无需复刻 run 切分。
pub(crate) fn multi_key_sort(
    idx: &[usize],
    sort_exps: &[Expr],
    is_descs: &[bool],
    dsn: &str,
    ctx: &mut dyn EvalCtx,
) -> Result<Vec<usize>> {
    // 逐行逐键求值（`SortDsValue.sort:361-378` 的 holder 键；DSVariable 快路径
    // 与表达式路径在本仓同形 —— `eval_ds_arg` 对裸列名就是取行值）。
    let saved = {
        let set = ctx.ds(dsn).ok_or_else(|| missing(dsn))?;
        set.current
    };
    let result: Result<Vec<Vec<Value>>> = (|| {
        let mut keys = Vec::with_capacity(idx.len());
        for &i in idx {
            ctx.ds(dsn).expect("上面刚取到").set_current(DsCur::Row(i));
            let mut ks = Vec::with_capacity(sort_exps.len());
            for e in sort_exps {
                ks.push(eval_bound(e, dsn, ctx)?);
            }
            keys.push(ks);
        }
        Ok(keys)
    })();
    ctx.ds(dsn).expect("同上").set_current(saved);
    let keys = result?;

    let mut pos: Vec<usize> = (0..idx.len()).collect();
    pos.sort_by(|&a, &b| {
        for k in 0..sort_exps.len() {
            // 比较器 = `Variant2.compare2`（`IIlIIIIIlIIllIII`/`IlIlIIllllIlIlII`），
            // 本仓对应 `aggregate_cmp`（Text-Text 走产品字典序）。
            let o = aggregate_cmp(&keys[a][k], &keys[b][k]).unwrap_or(Ordering::Equal);
            let o = if is_descs[k] { o.reverse() } else { o };
            if o != Ordering::Equal {
                return o;
            }
        }
        Ordering::Equal
    });
    Ok(pos.into_iter().map(|p| idx[p]).collect())
}
