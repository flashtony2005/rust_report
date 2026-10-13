//! DS 函数与 List 聚合（自 `expr/eval.rs` 拆出）。
//!
//! 涵盖：`ds.col`（DSVariable 求值）、`sum(...)` 的 List 聚合、
//! `ds.select/sum/count(...)` 双态分派、`evaluateRows` 逐行循环。
//! 事实源：附件 H.2（数据集双态）；`DSSelect.java:141-205`、
//! `DSSum.java:22-63`、`Row.java:97-110`、`Sum.java:117-133`。
//!
//! 循环依赖说明：`ds_call`/`eval_ds_arg` 要回调 [`super::eval::evaluate`]，
//! 而 `eval.rs` 的分发又要进来调 [`ds_call`] —— Rust 模块间互指是合法的，
//! 与产品里 `Expression.calculate ↔ DSSelect.calculate` 的互相调用同构。

use crate::error::{EngineError, Result};
use crate::expr::ast::{Call, Expr};
use crate::value::Value;

use super::ds::DsCur;
use super::eval::EvalCtx;
use super::num::num_add;

/// `ds.col`（DSVariable）：Row 态取当前行该列；Root 态产品的 Group 语义
/// 未核实且 12 夹具不用 → `UnsupportedExpr`（§13.5 同类，诚实不支持）。
pub(crate) fn ds_column(ctx: &mut dyn EvalCtx, ds: &str, col: &str) -> Result<Value> {
    // 借用期收缩：`ctx.ds()` 的 &mut 不能跨后续调用存活
    let idx = {
        let set = ctx.ds(ds).ok_or_else(|| EngineError::MissingDataset { name: ds.to_string() })?;
        set.col_index(col)
            .ok_or_else(|| EngineError::UnknownCell { id: format!("{ds}.{col}") })?
    };
    let set = ctx.ds(ds).expect("上面刚取到");
    Ok(match set.current {
        DsCur::Row(i) => set.rows.get(i).and_then(|r| r.get(idx)).cloned().unwrap_or(Value::Null),
        DsCur::Root => return Err(EngineError::UnsupportedExpr { expr: format!("{ds}.{col} (Root)") }),
    })
}

/// `Sum.calculate` 的 List 聚合（`Sum.java:117-133`）：跳 null、`Variant2.add`
/// 累加、全空 → `Double(0.0)`。
pub(crate) fn sum_list(list: Vec<Value>) -> Result<Value> {
    let mut acc: Option<Value> = None;
    for o in list {
        if o.is_null() {
            continue;
        }
        acc = Some(match acc {
            None => o,
            Some(a) => num_add(&a, &o)?,
        });
    }
    Ok(acc.unwrap_or(Value::Double(0.0)))
}

/// DS 调用参数的求值：裸列名（`Unknown`）绑定到**本调用**的数据集
/// （`UnknownSymbol.setDS` 的语义）；其余表达式原样求值。
pub(crate) fn eval_ds_arg(arg: &Expr, dsn: &str, ctx: &mut dyn EvalCtx) -> Result<Value> {
    match arg {
        Expr::Unknown(col) => ds_current_data(ctx, dsn, col),
        other => super::eval::evaluate(other, ctx),
    }
}

/// `DataSet.getCurrentData(colNo)`：当前 DsValue 的该列数据。
/// Row 态 → 当前行该列；Root 态 → 不支持（同 [`ds_column`]）。
pub(crate) fn ds_current_data(ctx: &mut dyn EvalCtx, dsn: &str, col: &str) -> Result<Value> {
    let idx = {
        let set = ctx.ds(dsn).ok_or_else(|| EngineError::MissingDataset { name: dsn.to_string() })?;
        set.col_index(col)
            .ok_or_else(|| EngineError::UnknownCell { id: format!("{dsn}.{col}") })?
    };
    let set = ctx.ds(dsn).expect("上面刚取到");
    Ok(match set.current {
        DsCur::Row(i) => set.rows.get(i).and_then(|r| r.get(idx)).cloned().unwrap_or(Value::Null),
        DsCur::Root => {
            return Err(EngineError::UnsupportedExpr { expr: format!("{dsn}.{col} (Root)") })
        }
    })
}

/// DS 函数（`ds.名(...)`）：select / sum / count。
///
/// Row 态（数据集被扩展格引用，指针随实例推进）返回当前行的值；
/// Root 态做全量收集/聚合 —— 附件 H.2 的双态语义。
pub(crate) fn ds_call(c: &Call, dsn: &str, ctx: &mut dyn EvalCtx) -> Result<Value> {
    let is_row = {
        let set = ctx.ds(dsn).ok_or_else(|| EngineError::MissingDataset { name: dsn.to_string() })?;
        matches!(set.current, DsCur::Row(_))
    };
    match (c.name.as_str(), is_row, c.args.len()) {
        // DSSelect Row 态（`DSSelect.java:143-149`）：setValue + 返回 Row ⇒ 拆包缓存值
        ("select", true, 1) => {
            let v = eval_ds_arg(&c.args[0], dsn, ctx)?;
            let i = {
                let set = ctx.ds(dsn).expect("上面刚取到");
                let i = set.current_row().unwrap_or(0);
                set.selected[i] = Some(v.clone());
                i
            };
            // ⛔ 旁路（§5.4 建模缺口）：产品此路返回的是 **Row 对象本身**
            // （`return cur;`，`DSSelect.java:149`），`ExtExtendCell.calculate:403/461`
            // 靠 `result instanceof DsValue` 把 `_$4` 定到这一行。Rust 的 [`Value`]
            // 是封闭九类、不携带身份 ⇒ 用 [`EvalCtx::note_ds_row`] 补记，
            // 由 `calc::cell_eval` 在求值后消费。
            if let Some(idx) = ctx.ds_index(dsn) {
                ctx.note_ds_row(idx, i);
            }
            Ok(ctx.ds(dsn).expect("同上").unwrap_row(i))
        }
        // DSSelect Root 态：全量 evaluateRows → List（`DSSelect.java:186-190`）
        ("select", false, 1) => {
            let rows = eval_rows(dsn, &c.args[0], ctx)?;
            // ⛔ 旁路：产品 Root 态返回 **List<Row>** —— `Group.getRows()`
            // （`Group.java:455-457`）返回的 `_$3` 元素全是 Row 对象
            // （`Group.evaluateRows:436-447` 逐元素 `(Row)this._$3.get(i)`），
            // 扩展格靠它逐元素写 `_$4`（`ExtExtendCell.calculate:401-407`）。
            // `eval_rows` 无 filter/distinct/sort ⇒ 元素 i 恒对应第 i 行。
            if let Some(idx) = ctx.ds_index(dsn) {
                let all: Vec<usize> = (0..rows.len()).collect();
                ctx.note_ds_list(idx, &all);
            }
            Ok(Value::List(rows))
        }
        // DSSum Row 态（`DSSum.java:41-47`）：同 select 的 Row 路径
        ("sum", true, 1) => {
            let v = eval_ds_arg(&c.args[0], dsn, ctx)?;
            let i = {
                let set = ctx.ds(dsn).expect("上面刚取到");
                let i = set.current_row().unwrap_or(0);
                set.selected[i] = Some(v.clone());
                i
            };
            // ⛔ 旁路：同 select Row 态（`DSSum.java:46` 同样 `return cur;`）。
            if let Some(idx) = ctx.ds_index(dsn) {
                ctx.note_ds_row(idx, i);
            }
            Ok(ctx.ds(dsn).expect("同上").unwrap_row(i))
        }
        // DSSum Root 态：全量和（`DSSum.java:50-61`）。⛔ 返回**标量**
        // （聚合值或 `Double(0.0)`），不是 Row 也不是 List ⇒ 不记旁路。
        ("sum", false, 1) => sum_list(eval_rows(dsn, &c.args[0], ctx)?),
        // DSCount Root 态：行数（夹具 ds2.count() → 3）
        ("count", false, 0) => {
            let set = ctx.ds(dsn).expect("上面刚取到");
            Ok(Value::Int(set.rows.len() as i32))
        }
        (name, _, n) => Err(EngineError::UnsupportedExpr {
            expr: format!("{dsn}.{name} 该形态未支持（{n} 参数）"),
        }),
    }
}

/// `Group.evaluateRows`：逐行把 ds 指针切到该行求值（`Row.java:97-110` 的循环
/// 形态；`Row.evaluate` 的"保存 → setCurrent → finally 恢复"展开成外层循环）。
pub(crate) fn eval_rows(dsn: &str, col_expr: &Expr, ctx: &mut dyn EvalCtx) -> Result<Vec<Value>> {
    let (n, saved) = {
        let set = ctx.ds(dsn).ok_or_else(|| EngineError::MissingDataset { name: dsn.to_string() })?;
        (set.rows.len(), set.current)
    };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        {
            let set = ctx.ds(dsn).expect("上面刚取到");
            set.set_current(DsCur::Row(i));
        }
        out.push(eval_ds_arg(col_expr, dsn, ctx)?);
    }
    ctx.ds(dsn).expect("上面刚取到").set_current(saved);
    Ok(out)
}
