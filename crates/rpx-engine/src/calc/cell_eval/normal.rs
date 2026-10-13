//! 普通格的求值尾段（`ExtNormalCell.calculate:652-675` 的 else 分支）。
//!
//! 与扩展格的两处**结构差异**（`extend.rs` 对照）：
//! 1. **没有 `_$4` / DS 指针操作** —— `ExtNormalCell` 压根没有 `_$4` 字段
//!    （`ds_current.rs` 模块文档第 3 条），DS 归属旁路在普通格**只记不消费**；
//! 2. disp 触发集是 `-64 || -58 || -56`（与扩展格**第一趟**相同、与扩展格
//!    **标量分支**的 `-64 || -58` 不同 —— 产品不一致，见 `extend.rs`）。
//!
//! # `Variant2.getValue(result)` 为什么是恒等
//!
//! 产品该调用做两件事：① 把 `DsValue` 拆成它的缓存值（`DSSelect` Row 路径
//! `return cur;` 返回的是 Row 对象）；② 其余类型原样返回。Rust 侧 ① 已在
//! [`crate::expr::dsfn`] 的求值里完成（Row 路径直接返回拆包值 + 旁路记身份），
//! ② 由 [`Value`] 的封闭九类天然承载 ⇒ 这里直接用求值结果。

use crate::calc::cell_eval::disp;
use crate::calc::ctx::CalcCtx;
use crate::eval::Flow;
use crate::value::Value;

/// 普通格取值写值 + dispValue（`ExtNormalCell.java:652-675`）。
pub(crate) fn normal_calculate(ctx: &mut CalcCtx, id: crate::arena::CellId, result: Value) -> Result<(), Flow> {
    let cell_type = ctx.cell_type(id);
    if cell_type == -60 {
        // :653-658：子报表（`getValue() instanceof ExtCellSet` + `getSubRptType()==2`
        // 时还有一套 `lIIlIIIIIIllIlIl` 联动）。12 夹具无子报表格 ⇒ 诚实不支持。
        return Err(Flow::Fatal(crate::error::EngineError::UnsupportedExpr {
            expr: "子报表格 (-60) 未建模".into(),
        }));
    }
    // :659-661：回滚点在 setValue **之前**记录。
    // `calc = !nonCalc()` 捕获"进入时是否已算过"——正常路径（nonCalc 为真）它是
    // false，state 8 重算路径它是 true；catch 回滚用它恢复 bit30。
    let old_val = ctx.value_of(id);
    let calc_before = !ctx.rt(id).is_some_and(|r| r.non_calc());
    // :662：`setValue(Variant2.getValue(result))` —— 写值 + 置 bit30（绑定语义）。
    ctx.set_value(id, result);
    // :663-675：仅三类"可见文本"格算 dispValue，且失败要**回滚**。
    if matches!(cell_type, -64 | -58 | -56) {
        let outcome = disp::calc_disp_value(ctx, id).map(|d| ctx.set_disp_value(id, d));
        if let Err(e) = outcome {
            // catch（:669-674）：先裸写旧值（不置位），再 `setCalc(calc)` ——
            // calc=false 时后者会把值清成 null（与产品逐行同构：正常路径回滚
            // 的最终形态就是"值 null + 未算过"）。
            ctx.write_value_raw(id, old_val);
            ctx.set_calc(id, calc_before);
            return Err(Flow::Fatal(e));
        }
    }
    Ok(())
}
