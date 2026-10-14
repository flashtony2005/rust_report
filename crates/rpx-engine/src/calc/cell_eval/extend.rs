//! 扩展格的求值尾段（`ExtExtendCell.calculate:376-483`）：List 分支（扩展）与
//! 标量分支。
//!
//! # List 分支的骨架（`:376-454`，原文顺序即契约）
//!
//! ```text
//! len == 0 → setValue(null); return                      :384-386
//! dispValList/formatList 仅当 cellType ∈ {-64,-58,-56}    :388-393
//! oldVal / calc（回滚点）；oldCurDs = _$4                 :395-398
//! oldDs = list[0] 是 DsValue 时取其数据集当前指针          :400-404
//! 第一趟：逐元素 _$4 = value; ds.setCurrent(value,true);
//!         setValue(拆包值); valList[i]; disp/format       :406-426
//! catch：cellValue = oldVal; setCalc(calc); 上抛          :427-430
//! finally：_$4 = oldCurDs; oldDs != null 时恢复 ds 指针    :431-435
//! extend = 18 ? extendRow(this,len) : extendCol(this,len) :437-442
//! 第二趟：cell = _$7[i]; cell.setValue(valList[i]);
//!         cs.setCurrent(cell); cell._$4 = value;
//!         cell.setDispValue/setFormat; 恢复 ds            :444-465
//! cs.setCurrent(this)                                     :466
//! ```
//!
//! # 与 Rust [`Value`] 的身份对接（§5.4 旁路）
//!
//! 产品里 `list.get(i)` 的元素是 **Row 对象**（`Group.getRows()` 返回的 `_$3`
//! 元素全是 Row，见 `dsfn.rs` 的旁路注释），`instanceof DsValue` 判定靠对象身份。
//! Rust 的 List 只带值 ⇒ 用 [`crate::expr::eval::EvalCtx::ds_list_note`]（标量分支用
//! `ds_row_note`）补回「第 i 个元素 == 第 ds 号数据集第 rows[i] 行」。
//!
//! ⚠ 已声明缺口（§5.4 同款）：表达式把 DS 调用**包在别的运算里**时（如
//! `1 + ds1.select(q)`），旁路仍会记下内层归属而结果的顶层身份其实不是 Row ——
//! 产品的对象身份模型不会犯此错。12 夹具的扩展锚点表达式都是裸 DS 调用，
//! 该缺口无夹具级信号； architect 审计复核项。
//!
//! # 与普通格的三处差异（`normal.rs` 对照）
//!
//! 1. 有 `_$4`（本格当前代表的实例）与逐元素的 DS 指针推进/恢复；
//! 2. **没有** `saveCurrent()`（见 `cell_eval.rs` 模块文档"产品不一致"）；
//! 3. 标量分支的 disp 触发集是 `-64 || -58`，**不含 -56**（`:461`，对照第一趟
//!    `:390` 的三元集）—— 产品不一致，照抄。

use crate::arena::CellId;
use crate::calc::cell_eval::disp;
use crate::calc::ctx::CalcCtx;
use crate::calc::ds_current::{DsCurrentHost, DsInstance};
use crate::error::EngineError;
use crate::eval::Flow;
use crate::expr::eval::EvalCtx;
use crate::value::Value;

/// 扩展格尾段分派（`:377` 的 `instanceof List`）。
pub(crate) fn extend_calculate(ctx: &mut CalcCtx, id: CellId, result: Value) -> Result<(), Flow> {
    match result {
        Value::List(list) => list_branch(ctx, id, list),
        other => scalar_branch(ctx, id, other),
    }
}

/// 标量分支（`:455-479`）：结果不是 List ⇒ 不扩展，只取值写值。
fn scalar_branch(ctx: &mut CalcCtx, id: CellId, result: Value) -> Result<(), Flow> {
    let cell_type = ctx.cell_type(id);
    let old_val = ctx.value_of(id);
    let calc_before = !ctx.rt(id).is_some_and(|r| r.non_calc());
    // :459：写值 + 置 bit30。
    ctx.set_value(id, result);
    // :460-470：DsValue 身份（旁路）+ disp（`-64 || -58`，不含 -56，见模块文档）。
    let note = ctx.ds_row_note();
    let mut saved: Option<(usize, Option<DsInstance>)> = None;
    if let Some((d, row)) = note {
        let prev = ctx.ds_spot(d).value; // :462 `ds = value.getDataSet().getCurrent()`
        let inst = DsInstance::new(d, row);
        if let Some(rt) = ctx.rt_mut(id) {
            rt.instance = Some(inst); // :463 `this._$4 = result`
        }
        ctx.ds_set_current(d, Some(inst), true); // :464
        saved = Some((d, prev));
    }
    let outcome = if matches!(cell_type, -64 | -58) {
        disp::calc_disp_value(ctx, id).map(|d| ctx.set_disp_value(id, d))
    } else {
        Ok(())
    };
    // finally（:476-478）：`ds != null` 才恢复 —— prev 是 None（进 try 前 current
    // 为 null）时不恢复。
    if let Some((d, Some(prev))) = saved {
        ctx.ds_set_current(d, Some(prev), true);
    }
    // catch（:471-474）：值与位回滚；⛔ `_$4` 与 ds 指针**不**回滚（finally 已按
    // 进入时状态恢复 ds，`_$4` 保留）。
    if let Err(e) = outcome {
        ctx.write_value_raw(id, old_val);
        ctx.set_calc(id, calc_before);
        return Err(Flow::Fatal(e));
    }
    Ok(())
}

/// List 分支（`:376-454`）：扩展锚点的主路径。
fn list_branch(ctx: &mut CalcCtx, id: CellId, list: Vec<Value>) -> Result<(), Flow> {
    let len = list.len();
    if len == 0 {
        // :384-386：空 List ⇒ 只清值。
        ctx.set_value(id, Value::Null);
        return Ok(());
    }
    let cell_type = ctx.cell_type(id);
    let want_disp = matches!(cell_type, -64 | -58 | -56); // :390 第一趟的三元集
    let old_val = ctx.value_of(id);
    let calc_before = !ctx.rt(id).is_some_and(|r| r.non_calc());
    let old_cur_ds = ctx.rt(id).and_then(|r| r.instance); // :398 `oldCurDs = this._$4`
                                                          // :400-404：`oldDs` 只在元素有 DsValue 身份时取（旁路有记录 ⟺ 是）。
    let note = ctx.ds_list_note();
    let old_ds = match &note {
        Some((d, _)) => ctx.ds_spot(*d).value,
        None => None,
    };
    // ── 第一趟（:406-426）──
    let mut val_list: Vec<Value> = Vec::with_capacity(len);
    let mut disp_list: Vec<Option<String>> = Vec::new();
    let first_pass: Result<(), EngineError> = (|| {
        for (i, v) in list.iter().enumerate() {
            if let Some((d, rows)) = &note {
                if let Some(&row) = rows.get(i) {
                    let inst = DsInstance::new(*d, row);
                    if let Some(rt) = ctx.rt_mut(id) {
                        rt.instance = Some(inst); // :410 `this._$4 = value`
                    }
                    ctx.ds_set_current(*d, Some(inst), true); // :411
                }
            }
            // :412：`setValue(Variant2.getValue(value))` —— 元素已是拆包值。
            ctx.set_value(id, v.clone());
            val_list.push(v.clone()); // :413 `valList[i] = this.cellValue`
            if want_disp {
                // :415 `dispValList[i] = this.calcDispValue(ctx)`（在元素 Row 态指针下求值；
                // 本 crate 的 calcDispValue 不含表达式求值 ⇒ 顺序无感，见 disp.rs）。
                disp_list.push(disp::calc_disp_value(ctx, id)?);
                // :416 `formatList[i] = this.getFormat()` —— format 未建模
                // （HeadCell 无该字段；12 夹具无 format 声明）⇒ 第二趟的
                // `cell.setFormat` 相应缺席。声明缺口。
            }
        }
        Ok(())
    })();
    // ── finally（:431-435）──
    if let Some(rt) = ctx.rt_mut(id) {
        rt.instance = old_cur_ds; // `this._$4 = oldCurDs`
    }
    if let (Some((d, _)), Some(prev)) = (&note, old_ds) {
        // `oldDs != null` 才恢复（:433-434）；old_ds 已拍平：None 同时覆盖
        // "元素无 DsValue 身份"与"进入时 current 为 null"两种不恢复情形。
        ctx.ds_set_current(*d, Some(prev), true);
    }
    // ── catch（:427-430）──
    if let Err(e) = first_pass {
        ctx.write_value_raw(id, old_val);
        ctx.set_calc(id, calc_before);
        return Err(Flow::Fatal(e));
    }
    // ── 扩展（:437-442）──
    let extend = ctx.grid.get(id).map(|h| h.extend).unwrap_or(16);
    let ext = if extend == 18 {
        crate::ext_algo::extend_row(&mut ctx.grid, id, len as i32)
    } else {
        crate::ext_algo::extend_col(&mut ctx.grid, id, len as i32)
    };
    ext.map_err(Flow::Fatal)?;
    // ── 第二趟（:444-465）──
    let members = EvalCtx::ext_cells(ctx, id); // `this._$7`（extend 后应有 len 个）
    for (i, &cid) in members.iter().enumerate().take(len) {
        // :447-448：克隆格写值 + `cs.setCurrent(cell)`。
        ctx.set_value(cid, val_list[i].clone());
        ctx.set_current(cid);
        // :450-455：元素身份写到**克隆**的 `_$4`，并推进/恢复 ds 指针。
        let saved2: Option<(usize, Option<DsInstance>)> = match (&note, list.get(i)) {
            (Some((d, rows)), Some(_)) => match rows.get(i) {
                Some(&row) => {
                    let prev = ctx.ds_spot(*d).value;
                    let inst = DsInstance::new(*d, row);
                    if let Some(rt) = ctx.rt_mut(cid) {
                        rt.instance = Some(inst); // :453 `cell._$4 = value`
                    }
                    ctx.ds_set_current(*d, Some(inst), true);
                    Some((*d, prev))
                }
                None => None,
            },
            _ => None,
        };
        if want_disp {
            // :456-458：克隆格的 dispValue 覆盖（`setDispValue` 内部"与声明不同才写"）。
            if let Some(dv) = disp_list.get(i).cloned() {
                ctx.set_disp_value(cid, dv);
            }
        }
        // finally（:461-463）：同第一趟，`ds != null` 才恢复。
        if let Some((d, Some(prev))) = saved2 {
            ctx.ds_set_current(d, Some(prev), true);
        }
    }
    // :466：`cs.setCurrent(this)`。
    ctx.set_current(id);
    Ok(())
}
