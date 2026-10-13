//! 顶层 `calculate()`：`ExtCellSet._$2(ctx):1483-1543` 的逐行移植 + 导出。
//!
//! 算法正文：`docs/m2-engine-design.md` **§5**（编排顺序表）、**§5.4**（接线约束）。
//! 编排原文（`:1485-1536`，顺序即契约）：
//!
//! ```text
//! if _$14 <= 4:                          // 入口守卫（幂等：state≥5 再进 = 无操作）
//!     prepareCalculate(ctx, true)        // 阶段 A+B
//!     _$21 = null; _$24 = new Queue()
//!     _$16/_$15 = 矩阵尺寸
//!     _$10.setValue(null)                // cell00 值清空（+bit30）
//!     ① _$1(ctx, dependCell)             // channel4（19 趟 → 18 趟）
//!     ② resetDSCurrent + setDSCurrent + _$19.reset()
//!     ③ _$2(ctx, special, dependCell)    // channel5（17 趟）
//!     ④ 同 ②
//!     ⑤ _$1(ctx, false)                  // calcProperties —— 未建模（缺口 1）
//!     ⑥ 样式宏 / 图表宏 / perfect —— 未建模（缺口 2）；仅移植 _$2() 尾列裁剪
//!     ⑦ 同 ②
//!     ⑧ _$1(ctx, special, dependCell)    // channel6（special 排空）
//!     _$14 = 7
//! ```
//!
//! # 两处已声明的建模缺口（12 夹具均不可达）
//!
//! 1. **calcProperties**（`:1188-1221`）驱动的是「值是 `ExtCellSet` 的格」
//!    （子报表/单元格图）与 printSetup 的 copyCountExp —— 夹具无此类格；
//! 2. **样式宏**（`_$1(_$9,ctx)`/`_$9()`/图表宏/`perfect`）—— 夹具无样式声明。
//!    `_$9()`（`:195-211`）是全表样式宏应用，**不在**通道循环里（本轮已核实：
//!    reportlite 版通道循环内逐格调的是 `_$8()` 中断检查，`sched.rs` 建模一致）。
//!
//! # 循环引用的 chain 诊断
//!
//! Java 的 `catch` 块把 `dependCell`（chase 栈）交给异常对象
//! （`((IIllIIIIIIIIIlll)e).setStack(dependCell, mm)`，`:1530-1532`），由诊断器
//! 拼出 `"B2-C2-B2-C2"` 形态的链。本 crate 在[`calculate`]的收口处同构拼装：
//! **chase 栈内容 + 抛出方**，`-` 连接，地址 = 列字母 + 行号。钉子见
//! `tests/engine_grid_compare.rs` 的 `engine_cycle` 断言（chain == `"B2-C2-B2-C2"`）。

use std::collections::VecDeque;

use rpx_core::ReportDefine;

use crate::arena::{CalcCell, CalcGrid, SourceRef, SubValue};
use crate::calc::ctx::CalcCtx;
use crate::calc::ds_current::{reset_ds_current, set_ds_current};
use crate::error::{EngineError, Result};
use crate::eval::sched::{channel4, channel5, channel6, DEFAULT_MAX_SPECIAL_REQUEUES};
use crate::eval::{finish_channel, state, Flow, Suspend};
use crate::extend::ExtendMode;
use crate::expr::ds::DataSet;
use crate::expr::eval::EvalCtx;
use crate::stage_b::prepare_calculate;
use crate::value::Value;

/// 引擎主入口：模板 + 数据集 → 计算态网格。
///
/// 一张 `HeadState` 只跑一次（`prepareCalculate` 的改写不可逆，`ctx.rs` 文档）；
/// 重入守卫 `_$14 <= 4` 在本形态下天然成立（每次新建 ctx，state 从 0 起）。
pub fn calculate(rd: &ReportDefine, ds: &[DataSet]) -> Result<CalcGrid> {
    let sb = prepare_calculate(rd)?;
    let mut ctx = CalcCtx::new(sb.grid, ds.to_vec());
    run_orchestration(&mut ctx)?;
    Ok(export_grid(&ctx))
}

/// `_$2(ctx)` 的主体（模块文档的 ①-⑧）。
fn run_orchestration(ctx: &mut CalcCtx) -> Result<()> {
    // `_$21 = null`（`:1490`）。
    ctx.set_current(crate::arena::CellId::NONE);
    // `_$24 = new Queue()`：失效队列每轮新建（`:1492`）。
    ctx.clear_fail_queue();
    // `_$10.setValue(null)`（`:1497`）：cell00 清值 + 置 bit30。
    ctx.set_value(crate::arena::CellId::ROOT, Value::Null);

    let mut st: Vec<crate::arena::CellId> = Vec::new();
    let mut special: VecDeque<crate::arena::CellId> = VecDeque::new();

    // ① channel4（`:1498`）。
    if let Err(flow) = channel4(ctx, &mut st) {
        return finish_with_chain(ctx, flow, &st);
    }
    interlude(ctx);
    // ③ channel5（`:1504`）。
    if let Err(flow) = channel5(ctx, &mut special, &mut st) {
        return finish_with_chain(ctx, flow, &st);
    }
    interlude(ctx);
    // ⑤ calcProperties、⑥ 样式宏：未建模（模块文档缺口 1/2）。
    // `_$2()` 尾列裁剪（`:1523`）—— 唯一本轮移植的 ⑥ 成员。
    trim_tail_cols(ctx);
    interlude(ctx);
    // ⑧ channel6（`:1533`）。
    if let Err(flow) = channel6(ctx, &mut special, &mut st, DEFAULT_MAX_SPECIAL_REQUEUES) {
        return finish_with_chain(ctx, flow, &st);
    }
    // `_$14 = 7`（`:1535`）。
    ctx.set_state(state::FINISHED);
    Ok(())
}

/// ②/④/⑦ 的三连：`resetDSCurrent + setDSCurrent + _$19.reset()`
/// （`:1499-1501`/`:1505-1507`/`:1527-1529`）。⚠ 不是 DS 推进器（`calc.rs` 文档）。
fn interlude(ctx: &mut CalcCtx) {
    reset_ds_current(ctx);
    set_ds_current(ctx);
    ctx.tracker.reset();
}

/// 错误收口：循环引用（state 1）按 chase 栈拼 chain；其余走 [`finish_channel`]。
fn finish_with_chain(ctx: &mut CalcCtx, flow: Flow, st: &[crate::arena::CellId]) -> Result<()> {
    if let Flow::Suspend(Suspend { cell, state: state::CIRCLE }) = flow {
        let mut names: Vec<String> = st.iter().map(|&c| addr_name(ctx, c)).collect();
        names.push(addr_name(ctx, cell));
        return Err(EngineError::CircularRef { chain: names.join("-") });
    }
    finish_channel(Err(flow))
}

/// `CellId` → `"B2"` 形态地址（列字母 + 行号；列 26 进制，A=1）。
fn addr_name(ctx: &CalcCtx, id: crate::arena::CellId) -> String {
    match ctx.grid.get(id) {
        Some(h) => {
            let (r, mut c) = (h.row, h.col);
            let mut letters = Vec::new();
            while c > 0 {
                letters.push((b'A' + ((c - 1) % 26) as u8) as char);
                c = (c - 1) / 26;
            }
            letters.reverse();
            format!("{}{}", letters.into_iter().collect::<String>(), r)
        }
        None => format!("cell#{}", id.0),
    }
}

/// `_$2()` 无参（`ExtCellSet.java:2175-2198`）：**尾部空列裁剪**。行永不裁。
///
/// 从最后一列向前扫，整列（i=1..rowSize-1）全 `null` 或 `cellType == -59` 才继续；
/// 停在最后一个有内容的列 `j`，删掉 `j+1..colSize-1`，并把横跨删除区的合并区
/// `endCol` 钳到 `j`。`j > 1`：第 1 列永不删。
fn trim_tail_cols(ctx: &mut CalcCtx) {
    let row_size = ctx.grid.row_size;
    let col_size = ctx.grid.col_size;
    if row_size <= 1 {
        return;
    }
    let mut j = col_size - 1;
    while j > 1 {
        let mut i = 1;
        let mut empty_or_blank = true;
        while i < row_size {
            match ctx.grid.by_pos.get(&(i, j)) {
                // `cell == null` ⇒ 该槽不算内容（`:2183` 的条件）。
                None => i += 1,
                Some(&id) => {
                    if ctx.grid.cells[id.0 as usize].rt.cell_type == -59 {
                        i += 1; // -59（空格）视作空
                    } else {
                        empty_or_blank = false;
                        break;
                    }
                }
            }
        }
        if !empty_or_blank {
            break; // `if (i != rowSize) break;`（`:2185`）
        }
        j -= 1;
    }
    if j < col_size - 1 {
        // `deleteCols(j + 1, colSize - j - 1)`（`:2187`）：摘掉这些列的槽位映射，
        // 尺寸缓存同步收缩（与 `insertRows/insertCols` 的 `+=` 维护同规）。
        for r in 1..row_size {
            for c in (j + 1)..col_size {
                ctx.grid.by_pos.remove(&(r, c));
            }
        }
        ctx.grid.col_size = j + 1;
        // 合并区钳制（`:2188-2194`）：`(i, j)` 上的格若合并区跨进删除区，
        // `endCol` 收到 `j`（即 `col_merge = j - col + 1`）。
        for i in 1..row_size {
            if let Some(&id) = ctx.grid.by_pos.get(&(i, j)) {
                let h = &mut ctx.grid.cells[id.0 as usize];
                if h.rt.cell_type == -59 {
                    continue;
                }
                let end_col = h.col + h.col_merge - 1;
                if end_col > j {
                    h.col_merge = j - h.col + 1;
                }
            }
        }
    }
}

/// 导出（`CalcJson.renderCalc:33-62` + `cellJson:64-119`）：稀疏收格。
///
/// ⛔ `source`/`sub_values` 读**引擎矩阵**（`by_src`），格本体读**计算矩阵**
/// （`by_pos`）—— 双矩阵契约（`arena.rs` 模块文档）。
fn export_grid(ctx: &CalcCtx) -> CalcGrid {
    let (row_size, col_size) = (ctx.grid.row_size, ctx.grid.col_size);
    let mut out = CalcGrid::new(row_size - 1, col_size - 1);
    for r in 1..row_size {
        for c in 1..col_size {
            let Some(&id) = ctx.grid.by_pos.get(&(r, c)) else { continue };
            let h = &ctx.grid.cells[id.0 as usize];
            let disp = h.disp_value();
            if h.value.is_null() && disp.is_none() {
                continue; // `v == null && dv == null ⇒ 不落盘`（:51-53）
            }
            let mut cell = CalcCell::bare(r, c, h.value.clone());
            cell.disp_value = disp;
            // left_head / top_head：head **目标**的模板出处（`headJson:129-134`）。
            if h.left_head.is_some() {
                cell.left_head = Some(ctx.grid.src_of(h.left_head));
            }
            if h.top_head.is_some() {
                cell.top_head = Some(ctx.grid.src_of(h.top_head));
            }
            // source：引擎矩阵在 (r, c) 上的一格（`ecs.getSource(r, c)`，:92）。
            if let Some(&sid) = ctx.grid.by_src.get(&(r, c)) {
                let s = &ctx.grid.cells[sid.0 as usize];
                let subs: Vec<crate::arena::CellId> = s
                    .ext_cells_ref()
                    .cloned()
                    .unwrap_or_default();
                let mut src = SourceRef::new(s.src_row, s.src_col,
                    ExtendMode::from_code(s.extend).unwrap_or(ExtendMode::Default16));
                src.row_merge = s.row_merge;
                src.col_merge = s.col_merge;
                src.sub_cells = subs.len().max(1) as i32; // `None 视作 1`（§7）
                // sub_values：实例的**计算态**坐标 + 当前值（`:104-113`）。
                cell.sub_values = subs
                    .iter()
                    .map(|&sub| {
                        let sh = &ctx.grid.cells[sub.0 as usize];
                        SubValue::new(sh.row, sh.col, sh.value.clone())
                    })
                    .collect();
                cell.src = Some(src);
            }
            out.cells.push(cell);
        }
    }
    out
}
