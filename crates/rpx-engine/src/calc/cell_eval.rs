//! 逐格 `calculate`（`ExtNormalCell.calculate:633-676` + `ExtExtendCell.calculate:355-483`）
//! 与 `testValue`（`ExtNormalCell.java:301-317`）的移植。
//!
//! 算法正文：`docs/m2-engine-design.md` **§5.3**（逐格求值）与 **§5.4**（DS 旁路）。
//! 文件切分：本文件只留**公共前置**（守卫序列 + 分派）与 [`test_value`]；
//! 普通格在 [`normal`]，扩展格在 [`extend`]，`calcDispValue` 在 [`disp`]。
//!
//! # 公共前置守卫的顺序（两类格**逐行相同**，唯一例外见下）
//!
//! ```text
//! if expression == null            → 什么都不做            :635 / :357
//! if engine.state > cs.getState()  → 挂起（要求进 state 通道）:637-639 / :359-361
//! if nonCalc() || (cs==8 && engine.state==8)              :641 / :363
//!     leftHead.testValue(); topHead.testValue()           :642-647 / :366-371
//!     [⛔ 仅普通格] cs.saveCurrent()                       :647（扩展格没有！:372-375 直接到下一句）
//!     if isDSFunction && isMainChanged() → setDSCurrent   :648-651 / :372-375
//!     result = expression.calculate(ctx)                  :653 / :377
//!     （普通格：直接取值写值；扩展格：List / 标量两分支）
//! ```
//!
//! ⛔ **`saveCurrent` 的有无是产品不一致**：`ExtNormalCell.java:647` 有、
//! `ExtExtendCell.calculate:372-375` **没有**（对照原文确认过）。它决定 `$B2`
//! （`getSavedCurrent()`）在扩展格里解析到的是**上一个 saveCurrent 者的格**而不是
//! 本格 —— 别"顺手补对称"。
//!
//! # state 8 分支
//!
//! `cs.getState()==8 && engine.state==8` 只在 `_$14==7 && (_$12&4)`（reflush）时可达
//! （`ExtCellSet.java:1702`），普通单次 calc 的 `_$14` 序列是 4/5/6/7 ⇒ 该分支
//! 在 12 夹具上恒不可达；保留它只为守卫语义完整（`runtime.rs` 模块文档同款偏离）。
//!
//! # `isDSFunction` 的 Rust 形态
//!
//! Java 在 `checkExtend`/`prepProperties` 时把"表达式含 DS 函数"算好存进
//! `engine.isDSFunction`（`ExtNormalCell.java:535-537`/`:581-585`）。本 crate 在
//! 每次进入 `cell_calculate` 时对（已 clone 的）AST 扫一遍 [`expr_has_ds_call`] ——
//! AST 构造后不变 ⇒ 与"算一次存下来"语义等价，代价是每次一次小遍历。

mod disp;
mod extend;
mod normal;

use crate::arena::CellId;
use crate::calc::ctx::CalcCtx;
use crate::error::EngineError;
use crate::eval::{state, Flow, Suspend};
use crate::expr::ast::Expr;
use crate::expr::eval::EvalCtx;
use crate::value::Value;

use super::ds_current::set_ds_current_for_cell;

/// 逐格 `calculate` 的分派入口（[`crate::eval::sched::SchedGrid::calculate`] 的落点）。
pub(crate) fn cell_calculate(ctx: &mut CalcCtx, id: CellId) -> Result<(), Flow> {
    // AST 先 clone 出来：后面每一步都要 `&mut ctx`，不能持着 grid 的借用。
    let Some(expr) = ctx.grid.get(id).and_then(|h| h.rt.expr.clone()) else {
        // `expression == null`：字面量格什么都不做（:635 / :357）。
        // 字面量格的"已求值"位已由阶段 B 置上（见 `stage_b` 的值写入注释）。
        return Ok(());
    };
    let (engine_state, is_extender) = match ctx.grid.get(id) {
        Some(h) => (h.rt.state, h.is_extender),
        None => return Ok(()),
    };
    let cs_state = ctx.get_state();
    if engine_state > cs_state {
        // :637-639 / :359-361：本格要的通道还没轮到 ⇒ 请求换格。
        return Err(Flow::Suspend(Suspend::new(id, engine_state)));
    }
    let non_calc = ctx.rt(id).is_some_and(|r| r.non_calc());
    let re_entry = cs_state == state::THIRD && engine_state == state::THIRD;
    if !(non_calc || re_entry) {
        // :641 / :363：已求值且不在 state 8 重算路径 ⇒ 无事可做。
        return Ok(());
    }
    // :642-647 / :366-371：先把两个 head 的值"顶"出来（它们可能反过来挂起本格）。
    let l = ctx.left_head(id);
    if l.is_some() {
        test_value(ctx, l, 0)?;
    }
    let t = ctx.top_head(id);
    if t.is_some() {
        test_value(ctx, t, 0)?;
    }
    // ⛔ saveCurrent 只有普通格做（模块文档"产品不一致"）。
    if !is_extender {
        ctx.save_current();
    }
    // :648-651 / :372-375：DS current 三件套（`isMainChanged` = `_$19.isChanged()`）。
    if expr_has_ds_call(&expr) && ctx.tracker.is_changed() {
        set_ds_current_for_cell(ctx, id)?;
    }
    // 每次求值前清空 DS 旁路 ⇒ 旁路里只留"这一次求值"的归属记录（§5.4）。
    ctx.clear_ds_notes();
    let result = match crate::expr::eval::evaluate(&expr, ctx) {
        Ok(v) => v,
        // testValue 的挂起从求值器深处穿过（搭载在 EngineError::Suspend 上，
        // 见 error.rs 文档）—— 在此还原成通道的 Flow::Suspend。
        Err(EngineError::Suspend(s)) => return Err(Flow::Suspend(s)),
        Err(e) => return Err(Flow::Fatal(e)),
    };
    if is_extender {
        extend::extend_calculate(ctx, id, result)
    } else {
        normal::normal_calculate(ctx, id, result)
    }
}

/// `testValue()`（`ExtNormalCell.java:301-317`，`ExtExtendCell` 同文）。
///
/// ⛔ 三个容易写错的点：
/// 1. `rowMerge > 0` ⟺ **bit31 未置**（位布局：逻辑跨度恒正、bit30 非负、bit31 置位
///    即为负）⇒ 对应 [`crate::heads::CellRuntime::demanded`] 取反；
/// 2. `rowMerge > 0` 分支置完 bit31 之后**仍要抛**（`:310`）—— 不是"置位后继续"；
///    抛的状态是 `engine.state`（请求调度器切到那个通道），不是 1；
/// 3. `rowMerge <= 0`（已被要求过一次）⇒ 抛 **state 1**（CIRCLE，循环引用，
///    `flow_to_result` 映射为 [`EngineError::CircularRef`]）。
///
/// 无表达式的格：`setValue(null)`（写 null 值 + 置 bit30，`:312`）—— 这是
/// "空格被依赖时也算它一次"的机制；字面量格因阶段 B 已置位而**不会**进这条
/// （见 `stage_b` 值写入注释）。
///
/// 末尾**无条件**沿 left/top 链递归（`:314-316`）—— 即使本格已算过。
/// `depth` 是防御上限（Java 靠 head 图无环终止）。
pub(crate) fn test_value(ctx: &mut CalcCtx, id: CellId, depth: u32) -> Result<(), Flow> {
    if depth > 512 {
        return Err(Flow::Fatal(EngineError::Invariant {
            detail: format!("testValue 链过深（cell#{}），head 图疑似有环", id.0),
        }));
    }
    let (non_calc, has_expr, engine_state, demanded) = match ctx.grid.get(id) {
        Some(h) => (
            h.rt.non_calc(),
            h.rt.expr.is_some(),
            h.rt.state,
            h.rt.demanded,
        ),
        None => return Ok(()),
    };
    if non_calc {
        if has_expr {
            if demanded {
                // :307-309：第二次被要求 ⇒ 循环引用。
                return Err(Flow::Suspend(Suspend::new(id, state::CIRCLE)));
            }
            // :305：置 bit31（"已被要求求值"）。
            if let Some(rt) = ctx.rt_mut(id) {
                rt.demanded = true;
            }
            // :310：仍要抛 —— 请求调度器切到 engine.state 通道。
            return Err(Flow::Suspend(Suspend::new(id, engine_state)));
        }
        // :312：无表达式 ⇒ 写 null（写值即置 bit30）。
        ctx.set_value(id, Value::Null);
    }
    // :314-316：无条件递归。
    let l = ctx.left_head(id);
    if l.is_some() {
        test_value(ctx, l, depth + 1)?;
    }
    let t = ctx.top_head(id);
    if t.is_some() {
        test_value(ctx, t, depth + 1)?;
    }
    Ok(())
}

/// `engine.isDSFunction` 的对应物：AST 里是否有 DS 函数调用
/// （`ds.xxx(...)` 带限定符的 [`Expr::Call`]，或 [`Expr::DsColumn`]）。
fn expr_has_ds_call(e: &Expr) -> bool {
    match e {
        Expr::Call(c) => c.qualifier.is_some() || c.args.iter().any(expr_has_ds_call),
        Expr::DsColumn { .. } => true,
        Expr::Unary { rhs, .. } => expr_has_ds_call(rhs),
        Expr::Binary { lhs, rhs, .. } => expr_has_ds_call(lhs) || expr_has_ds_call(rhs),
        Expr::ValueList { items, .. } => items.iter().any(expr_has_ds_call),
        Expr::Select { arg } => expr_has_ds_call(arg),
        _ => false,
    }
}
