//! 求值器：AST → `Value`（§12.1 第 9 步切片）。
//!
//! Java 对应：`Expression.calculate`（逐节点 `calculate(ctx)`）+ 各函数类。
//! 附件 H.1/H.2 是求值语义的事实源（⛔ **不是 Excel 语义**）。
//!
//! 文件按职责拆分：本文件只留 [`EvalCtx`] trait、分发入口 [`evaluate`]、
//! 函数/一元/二元分发与 `sum(A:B)` 的 [`sum_link`]；DS 函数与 List 聚合在
//! [`dsfn`]，数值原语（`Variant2.*` 移植）在 [`num`]。
//!
//! # 本切片已实现（12 夹具的完整表达式面，`EngineModels.java` 全量清单）
//!
//! | 形态 | 语义 | 出处 |
//! |------|------|------|
//! | `B2` / `$B2` | CSVariable 相对引用（`resolve.rs`） | `CSVariable.java` |
//! | `B2*2` / `6*1` / … | 算术，**类型保真**（Int 不提升） | 附件 H.3 |
//! | `ds1.select(col)` | Row 态：当前行值；Root 态：整列 List | `DSSelect.java:141-205` |
//! | `ds1.sum(col)` | Row 态：当前行值；Root 态：全量和 | `DSSum.java:22-63` |
//! | `ds1.count()` | Root 态：行数 | `DSCount` |
//! | `sum(x)` | List → 聚合；标量 → 透传；空 → `Double(0.0)` | `Sum.java:106-133` |
//! | `sum(A:B)` | ⛔ **不是区域求和**：把 B 设为 current 后求值 A 一次 | `Sum.java:174-206` |
//!
//! # 求值上下文 [`EvalCtx`]
//!
//! 产品里这些能力散在 `ExtCellSet`（current 两指针、`getSource`）与格对象
//! （head 链、实例数组）上；这里收拢为一个 trait，让单测可以注入假实现，
//! 下一接线步由 `HeadState` 实现生产版。CSVariable 的解析算法在
//! [`super::resolve`] 里以 `&dyn EvalCtx` 为参数 —— 不依赖具体网格类型。
//!
//! # 已知边界（诚实声明）
//!
//! - `avg`/`max`/`min`/`group`/`sort`/`Move`/`Offset`/`Select{}` 的 `left`、
//!   区间与 `In`/`Eval` 运算之外的面 → `UnsupportedExpr`；
//! - `firstLeft` 序列化门、`option 'r'`、`num/distinct` 参数未建模（夹具不用）。

use crate::arena::CellId;
use crate::error::{EngineError, Result};
use crate::expr::ast::{BinOp, Call, Expr, UnOp};
use crate::value::Value;

use super::ds::DataSet;
use super::dsfn::{avg_list, ds_call, ds_column, max_list, min_list, sum_list};
use super::num::{as_bool, num_add, num_cmp, num_div, num_mod, num_mul, num_neg, num_sub, val_eq};

/// 求值上下文（见模块文档）。
pub trait EvalCtx {
    // ── head 图原语（CSVariable 解析所需；`ExtCell` 的可观察面）──
    /// 实例的引擎出处（`getSource()`；引擎矩阵本体格返回自身）。
    fn source_of(&self, c: CellId) -> CellId;
    fn left_head(&self, c: CellId) -> CellId;
    fn top_head(&self, c: CellId) -> CellId;
    /// 左/上子格列表（`_$6`/`_$5`；仅扩展格与 root 非空）。
    fn left_subs(&self, c: CellId) -> Vec<CellId>;
    fn top_subs(&self, c: CellId) -> Vec<CellId>;
    /// 扩展实例数组（`_$7`；非扩展格为空）。
    fn ext_cells(&self, c: CellId) -> Vec<CellId>;
    /// 生效 extensible（`getExtensible()`）。
    fn extensible(&self, c: CellId) -> i8;
    /// `cell00`。
    fn root(&self) -> CellId;
    // ── current 两指针与取值（`ExtCellSet.java:578-596`）──
    /// `getCurrent()`（`_$21`）：**当前正在求值的格**。相对引用的基准。
    fn current(&self) -> CellId;
    /// `getSavedCurrent()`（`_$20`）：`saveCurrent()` 存下的那一份。`$B2` 用的是它。
    fn saved_current(&self) -> CellId;
    /// `setCurrent(c)`（`:578-580`）：只写 `_$21`。
    fn set_current(&mut self, c: CellId);
    /// `saveCurrent()`（`:586-588`）：`_$20 = _$21`。
    ///
    /// ⛔ 只有 `ExtNormalCell.calculate:647` 与 `Sum` 的 cellSet 分支（`Sum.java:150`）
    /// 会调它 —— 不要"顺手"在别处补，`_$20` 被覆盖会让外层 `resumeCurrent` 回错格。
    fn save_current(&mut self);
    /// `resumeCurrent()`（`:594-596`）：`_$21 = _$20`。
    fn resume_current(&mut self);
    /// 实例当前值（`getValue(false)`；未算 → `Null`）。
    fn cell_value(&self, c: CellId) -> Value;
    /// 实例当前值，**先过 `testValue()` 门**（`ExtNormalCell.getValue:326-336`）。
    ///
    /// ⛔ 产品的 `getValue(boolean)` **无条件调 `this.testValue()`**（`:334`）——
    /// 未求值的格被引用 ⇒ 置 bit31 并抛挂起（依赖驱动的 chase 求值）；
    /// 第二次被引用 ⇒ state 1（CIRCLE，循环引用检测的机制本体）。
    /// [`Self::cell_value`] 的裸读形态只是给**假实现**与 `resolve` 内部用的；
    /// 引用取值路径（`Expr::CellRef`）必须走本方法。
    /// 默认实现 = 裸读（不建模依赖检测）；生产版由 `CalcCtx` 实现。
    fn cell_value_checked(&mut self, c: CellId) -> Result<Value> {
        Ok(self.cell_value(c))
    }
    /// `getSource(id)`：按格号查**引擎矩阵**；不存在 → `None`。
    fn source_by_id(&self, id: &str) -> Option<CellId>;
    // ── 数据集与参数 ──
    fn ds(&mut self, name: &str) -> Option<&mut DataSet>;
    /// 数据集名 → 稳定下标（DS 旁路与 `DsCurrentHost` 都要用它寻址）。
    fn ds_index(&self, name: &str) -> Option<usize>;
    fn param(&self, name: &str, is_macro: bool) -> Option<Value>;
    // ── DS 值归属旁路（见 `docs/m2-engine-design.md` §5.4 的建模缺口）──
    /// 记下「本次表达式求值的最终结果就是**第 `ds` 号数据集的第 `row` 行**」。
    ///
    /// 对应 Java 里 `ds.select(col)` / `ds.sum(col)` 在 **Row 态**返回 `DsValue`
    /// 对象本身（`DSSelect.java:145-149`），而 `ExtExtendCell.calculate:403/461`
    /// 靠 `result instanceof DsValue` 把 `_$4` 定位到行。Rust 的 [`Value`] 是
    /// **封闭的九类值**、不承载运行时对象 ⇒ 用这条旁路把那层"身份"补回来。
    fn note_ds_row(&mut self, _ds: usize, _row: usize) {}
    /// 记下「本次求值的最终结果是**第 `ds` 号数据集的全量行**，逐元素行号如 `rows`」。
    fn note_ds_list(&mut self, _ds: usize, _rows: &[usize]) {}
    /// 每次逐格求值**开始前**清空旁路，这样只会留下"这一次求值"的记录。
    fn clear_ds_notes(&mut self) {}
    /// 取出本次求值记下的"单个行"归属（若有）。
    fn ds_row_note(&self) -> Option<(usize, usize)> {
        None
    }
    /// 取出本次求值记下的"整列"归属（若有）。
    fn ds_list_note(&self) -> Option<(usize, Vec<usize>)> {
        None
    }
}

/// 求值入口（`Expression.calculate(ctx)` 的 Rust 形态）。
pub fn evaluate(expr: &Expr, ctx: &mut dyn EvalCtx) -> Result<Value> {
    match expr {
        Expr::Const(v) => Ok(v.clone()),
        // CSVariable（`B2` / `$B2`）：解析到实例 → 过 testValue 门再取值
        // （`getValue(false):334` 无条件 testValue —— 见 `cell_value_checked` 文档）
        Expr::CellRef {
            id,
            absolute,
            provisional: _,
        } => {
            match super::resolve::resolve_cell(ctx, id, *absolute) {
                Some(cell) => ctx.cell_value_checked(cell),
                None => Ok(Value::Null), // Java getCell()==null ⇒ getValue 返回 null
            }
        }
        // CSCurrent（`@`）：当前格的值
        Expr::CurrentCell => Ok(ctx.cell_value(ctx.current())),
        // ParamNode / Constant(参数值)
        Expr::Param {
            name,
            is_macro,
            value,
        } => match value {
            Some(v) => Ok(v.clone()),
            None => ctx
                .param(name, *is_macro)
                .ok_or_else(|| EngineError::UnknownCell {
                    id: format!("@{name}"),
                }),
        },
        // DSVariable（`ds1.qty`）：Row 态取当前行；Root 态产品语义未核实（未用）
        Expr::DsColumn { ds, col } => ds_column(ctx, ds, col),
        // UnknownSymbol：DS 调用参数里的裸列名由该 DS 绑定数据集（`setDS` +
        // `ds.getCurrentData(colNo)`，`UnknownSymbol.java:32-37/48-51`，见
        // `dsfn::eval_ds_arg`）；独立出现的 Unknown 产品会抛 unknownExpression。
        Expr::Unknown(name) => Err(EngineError::UnsupportedExpr { expr: name.clone() }),
        // ValueList（`[...]` / `list(...)`）：逐项求值成 List
        Expr::ValueList { items, .. } => {
            let mut vs = Vec::with_capacity(items.len());
            for it in items {
                vs.push(evaluate(it, ctx)?);
            }
            Ok(Value::List(vs))
        }
        // Select{}：花括号内容求值（`left` 语义未建模，模块文档已声明）
        Expr::Select { arg } => evaluate(arg, ctx),
        // Move/Offset：未支持
        Expr::Index { .. } => Err(EngineError::UnsupportedExpr {
            expr: "Move/Offset".into(),
        }),
        Expr::Call(c) => evaluate_call(c, ctx),
        Expr::Unary { op, rhs } => evaluate_unary(*op, rhs, ctx),
        Expr::Binary { op, lhs, rhs } => evaluate_binary(*op, lhs, rhs, ctx),
    }
}

fn evaluate_call(c: &Call, ctx: &mut dyn EvalCtx) -> Result<Value> {
    // ── DS 函数（`ds.名(...)`）──
    if let Some(dsn) = &c.qualifier {
        return ds_call(c, dsn, ctx);
    }
    match c.name.as_str() {
        // ── sum：叶子 / 复合两路（`Sum.java:106-206`）──
        "sum" if c.param_kind == crate::expr::ast::ParamKind::Colon && c.args.len() == 2 => {
            sum_link(c, ctx)
        }
        "sum" if c.args.len() == 1 => {
            let v = evaluate(&c.args[0], ctx)?;
            if v.is_null() {
                return Ok(Value::Double(0.0)); // `result1 == null → Double(0.0)`（`:111-113`）
            }
            match v {
                Value::List(l) => sum_list(l),
                scalar => Ok(scalar), // 标量透传（`:115-117`）
            }
        }
        // ── M2-12：avg / max / min（`Average.java:100-125` / `Max:100-121` /
        //    `Min:100-121`）。三者的叶子路径同构：标量透传、List 才聚合。
        "avg" if c.args.len() == 1 => {
            let v = evaluate(&c.args[0], ctx)?;
            match v {
                Value::List(l) => avg_list(&l, c.option.as_deref().unwrap_or("").contains('a')),
                scalar => Ok(scalar), // `else { return result1; }`（`:120-122`）
            }
        }
        "max" if c.args.len() == 1 => {
            let v = evaluate(&c.args[0], ctx)?;
            match v {
                Value::List(l) => max_list(&l),
                scalar => Ok(scalar), // `else { result = result1; }`（`:115-117`）
            }
        }
        "min" if c.args.len() == 1 => {
            let v = evaluate(&c.args[0], ctx)?;
            match v {
                Value::List(l) => min_list(&l),
                scalar => Ok(scalar), // 同 max
            }
        }
        other => Err(EngineError::UnsupportedExpr {
            expr: format!("{other}(...)"),
        }),
    }
}

/// `sum(A:B)` 的 cellSet 分支（`Sum.java:174-206`）：**先 `saveCurrent()`，把 B 设为
/// current，求值 A 一次，`finally` 里 `resumeCurrent()`**。
///
/// ⛔ 两个都不能省：
/// - `saveCurrent()` 把 `_$20` 覆盖成"进来时的 `_$21`"，于是 `resumeCurrent()` 回得对；
///   只读 `saved_current()` 而不先存，遇到嵌套 `sum()` 会回到**外层**的格。
/// - 只有单个格时才走这条；`sub1` 求值成 `List`（多格区域）时产品逐格循环
///   （`Sum.java:161-175`），本 crate 尚无夹具覆盖该形态 ⇒ 报 `UnsupportedExpr`。
fn sum_link(c: &Call, ctx: &mut dyn EvalCtx) -> Result<Value> {
    // cellSet = sub1（`Sum.java:177`）：必须是可解析到格的表达式
    let cell = match &c.args[1] {
        Expr::CellRef {
            id,
            absolute,
            provisional: _,
        } => super::resolve::resolve_cell(ctx, id, *absolute)
            .ok_or_else(|| EngineError::UnknownCell { id: id.clone() })?,
        other => {
            return Err(EngineError::UnsupportedExpr {
                expr: format!("sum cellSet 不是格引用: {other:?}"),
            })
        }
    };
    // `ecs.saveCurrent()`（`:150`）
    ctx.save_current();
    ctx.set_current(cell); // `ecs.setCurrent(cell)`（`:198`）
    let r = evaluate(&c.args[0], ctx);
    // `finally { ecs.resumeCurrent() }`（`:204-206`）——即使求值失败也要回卷
    ctx.resume_current();
    let v = r?;
    Ok(if v.is_null() { Value::Double(0.0) } else { v })
}

fn evaluate_unary(op: UnOp, rhs: &Expr, ctx: &mut dyn EvalCtx) -> Result<Value> {
    let v = evaluate(rhs, ctx)?;
    match op {
        UnOp::Neg => num_neg(&v),
        UnOp::Pos => Ok(v),
        UnOp::Not => Ok(Value::Bool(!as_bool(&v)?)),
        UnOp::Address => Err(EngineError::UnsupportedExpr {
            expr: "&addr".into(),
        }),
    }
}

fn evaluate_binary(op: BinOp, lhs: &Expr, rhs: &Expr, ctx: &mut dyn EvalCtx) -> Result<Value> {
    let (a, b) = (evaluate(lhs, ctx)?, evaluate(rhs, ctx)?);
    match op {
        BinOp::Add => num_add(&a, &b),
        BinOp::Sub => num_sub(&a, &b),
        BinOp::Mul => num_mul(&a, &b),
        BinOp::Div => num_div(&a, &b),
        BinOp::Mod => num_mod(&a, &b),
        BinOp::Eq => Ok(Value::Bool(val_eq(&a, &b))),
        BinOp::Ne => Ok(Value::Bool(!val_eq(&a, &b))),
        BinOp::Gt => Ok(Value::Bool(num_cmp(&a, &b)? == std::cmp::Ordering::Greater)),
        BinOp::Ge => Ok(Value::Bool(num_cmp(&a, &b)? != std::cmp::Ordering::Less)),
        BinOp::Lt => Ok(Value::Bool(num_cmp(&a, &b)? == std::cmp::Ordering::Less)),
        BinOp::Le => Ok(Value::Bool(num_cmp(&a, &b)? != std::cmp::Ordering::Greater)),
        BinOp::And => Ok(Value::Bool(as_bool(&a)? && as_bool(&b)?)),
        BinOp::Or => Ok(Value::Bool(as_bool(&a)? || as_bool(&b)?)),
        // Link 单独出现（`A:B` 不在 sum 里）——产品语义是"复合参数"，无独立求值面
        BinOp::Link | BinOp::Comma | BinOp::In | BinOp::Eval => Err(EngineError::UnsupportedExpr {
            expr: format!("{op:?}"),
        }),
    }
}
