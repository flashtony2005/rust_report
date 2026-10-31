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
//!     ⑤ _$1(ctx, false)                  // calcProperties（已有 expMap 内置属性）
//!     ⑥ 样式宏 / 图表宏 / perfect —— 未建模；已移植 _$2() 尾列裁剪
//!     ⑦ 同 ②
//!     ⑧ _$1(ctx, special, dependCell)    // channel6（special 排空）
//!     _$14 = 7
//! ```
//!
//! # 保留的建模缺口
//!
//! 1. `calcProperties` 已计算行/列/普通格 `expMap` 中可建模的内置属性表达式；独立
//!    `customPropEngMap`、ExtCellSet/cell-graph 值对象及其属性写回仍无模型或夹具；
//! 2. **样式宏**（`_$1(_$9,ctx)`/`_$9()`/图表宏/`perfect`）仍未建模，夹具无样式声明。
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
use crate::calc::chain::src_addr_name;
use crate::calc::ctx::CalcCtx;
use crate::calc::ds_current::{reset_ds_current, set_ds_current};
use crate::error::{EngineError, Result};
use crate::eval::sched::{channel4, channel5, channel6, DEFAULT_MAX_SPECIAL_REQUEUES};
use crate::eval::{finish_channel, state, Flow, Suspend};
use crate::expr::ds::DataSet;
use crate::expr::eval::EvalCtx;
use crate::extend::ExtendMode;
use crate::params::ParamTable;
use crate::stage_b::prepare_calculate_with;
use crate::value::Value;

/// 引擎主入口：模板 + 数据集 → 计算态网格。
///
/// **无参数表**版本（等价 [`ParamTable::EMPTY`]）：模板表达式里出现 `@名` 会在
/// 建树期报 [`EngineError::UnknownParam`]（显式失败，不静默当无参数算）。
/// 带参数的调用点（`/render`、CLI `--param`）用 [`calculate_with`]。
pub fn calculate(rd: &ReportDefine, ds: &[DataSet]) -> Result<CalcGrid> {
    calculate_with(rd, ds, &ParamTable::EMPTY)
}

/// 引擎主入口（M3-01 参数通道）：`params` 里的参数值在阶段 B 建树期内联进表达式
/// （与产品同为 `Constant(值)`，见 [`crate::params`]），驱动扩展与求值。
///
/// ⚠ `params` 是**输入的一部分**：同一模板 + 数据集，参数不同 ⇒ 计算矩阵可能不同
/// （List 参数会把格包成扩展格，真机语义见 RUNLOG §15/§24.3）。
///
/// 一张 `HeadState` 只跑一次（`prepareCalculate` 的改写不可逆，`ctx.rs` 文档）；
/// 重入守卫 `_$14 <= 4` 在本形态下天然成立（每次新建 ctx，state 从 0 起）。
pub fn calculate_with(rd: &ReportDefine, ds: &[DataSet], params: &ParamTable) -> Result<CalcGrid> {
    calculate_full(rd, ds, params, &crate::macros::MacroTable::EMPTY)
}

/// 引擎主入口（**全输入**形态）：参数表 + 宏表。
///
/// `macros` 是报表宏表（M7-8）。宏是"没有数据类型的字符串"，替换发生在**建表达式
/// 之前**（`MacroResolver2.replaceMacros` 在 `new Expression(...)` 之前一行，
/// `docs/m2-engine-design.md:1585`）⇒ 由 [`prepare_calculate_with`] 在两个建树点
/// 之前跑 [`crate::macros::substitute`]：单元格表达式与属性表达式。
///
/// ⛔ 宏替换**不覆盖 SQL 文本**：数据集 SQL 在本函数**之外**取数（服务面
/// `prefetch_sql`），它的宏替换由调用方在发 SQL 之前做 —— 见
/// [`crate::macros`] 模块文档的"可放任何位置"那条。
pub fn calculate_full(
    rd: &ReportDefine,
    ds: &[DataSet],
    params: &ParamTable,
    macros: &crate::macros::MacroTable,
) -> Result<CalcGrid> {
    let mut table = crate::macros::MacroTable::from_report(rd);
    table.overlay(macros);
    let macros = &table;
    // 输入校验先于扩展；模板分页与探针分页分别保留其观测口径。
    crate::pager::ensure_supported(&rd.print_setup)?;
    crate::pager::ensure_data_columns(rd)?;
    let sb = prepare_calculate_with(rd, params, macros)?;
    let mut ctx = CalcCtx::new(sb.grid, ds.to_vec());
    run_orchestration(&mut ctx)?;
    // 合并两趟。产品在 `SimplePager.createPages()` 里调（`:133-134` 单页 / `:180-190` 多页），
    // **不在** `Engine.calc()` 里 —— 本 crate 同址：合并跑在**整张计算矩阵**上
    // （不逐页跑）。合并区的**两个视图**（M5-02）在分页后经
    // [`crate::calc::merged_view`] 一次挂上：`merged` = 全局逐槽（服务面快照
    // `rpx-server` 的 `merge_data` + 跨页拆分吃它），`merged_pages` = 页内逐槽
    // （emit 对拍吃它，产品 `CalcMergeJson.renderMerged` 读的是**页**格）。
    // 顺序是契约：先冻结引擎矩阵侧的跨度，再 `mergeSame`、后 `mergeNull`（`:133-134`）。
    ctx.grid.freeze_engine_spans();
    crate::merge::merge_same(&mut ctx.grid);
    crate::merge::merge_null(&mut ctx.grid);
    let (mut out, areas) = export_grid(&ctx, rd);
    super::print_config::apply(&mut ctx, rd, &mut out, params, macros)?;
    out.graphics = crate::graphics::collect(&mut ctx, rd, params, macros)?;
    super::print_config::fit_graphics(&mut out);
    out.page_background = rd
        .custom_properties
        .iter()
        .find(|(key, _)| key == crate::graphics::BACKGROUND)
        .and_then(|(_, value)| value.clone());
    // 分页（M2-11）：`SimplePager.createPages()` 的探针口径（595×842pt、1 栏）。
    let pg = {
        let pg = crate::pager::paginate(
            &out,
            crate::pager::PAPER_W_PT,
            crate::pager::PAPER_H_PT,
            crate::pager::COLUMNS,
        );
        out.page_count = pg.page_count();
        // M5-02：合并区双视图（全局逐槽 + 页内逐槽）—— 页内视图即产品「跨页拆分」
        // （`SimplePager._$1:1624-1638` 行向 / `:2316-2335` 列向）的等价建模。
        crate::calc::merged_view::attach(&mut out, &areas, &pg);
        pg
    };
    out.pagination = Some(pg);
    Ok(out)
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
    // ⑤ calcProperties（`:1188-1221`）：行轴 → 列轴 → 普通格。
    crate::calc::properties::calculate_properties(ctx)?;
    // ⑥ 样式宏仍未建模；`_$2()` 尾列裁剪（`:1523`）是此阶段已移植的部分。
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
    if let Flow::Suspend(Suspend {
        cell,
        state: state::CIRCLE,
    }) = flow
    {
        // M5-09：链上的名字用**模板格号**（`h.src_row/src_col`），与产品一致；
        // 计算态地址会因扩展而偏移（模板 C2 落到 D2），产品报的仍是 C2。
        let mut names: Vec<String> = st.iter().map(|&c| src_addr_name(ctx, c)).collect();
        names.push(src_addr_name(ctx, cell));
        // 自引用时引擎栈会把同一格压多层（C2-C2-C2），产品只报两跳（C2-C2）：
        // 连续同格折叠；折叠后只剩一格则补成两跳（互引链 B2-C2-B2-C2 不受影响）。
        let mut chain: Vec<String> = Vec::new();
        for n in names {
            if chain.last() != Some(&n) {
                chain.push(n);
            }
        }
        if chain.len() == 1 {
            let n = chain[0].clone();
            chain.push(n);
        }
        return Err(EngineError::CircularRef {
            chain: chain.join("-"),
        });
    }
    finish_channel(Err(flow))
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

/// 导出（`CalcJson.renderCalc:33-62` + `cellJson:64-119`）：稀疏收格 + 几何。
///
/// 返回 `(网格, 合并区列表)` —— 合并区的**两个视图**（全局逐槽 / 页内逐槽）
/// 由调用方在分页后经 [`crate::calc::merged_view`] 挂上（M5-02）。
///
/// ⛔ `source`/`sub_values` 读**引擎矩阵**（`by_src`），格本体读**计算矩阵**
/// （`by_pos`）—— 双矩阵契约（`arena.rs` 模块文档）。
fn export_grid(
    ctx: &CalcCtx,
    rd: &ReportDefine,
) -> (CalcGrid, Vec<crate::calc::merged_view::MergedArea>) {
    let (row_size, col_size) = (ctx.grid.row_size, ctx.grid.col_size);
    let mut out = CalcGrid::new(row_size - 1, col_size - 1);
    out.unit = rd.scalars.unit;
    fill_geometry(ctx, rd, &mut out);
    for r in 1..row_size {
        for c in 1..col_size {
            let Some(&id) = ctx.grid.by_pos.get(&(r, c)) else {
                continue;
            };
            let h = &ctx.grid.cells[id.0 as usize];
            let disp = h.disp_value();
            if h.value.is_null() && disp.is_none() {
                continue; // `v == null && dv == null ⇒ 不落盘`（:51-53）
            }
            let mut cell = CalcCell::bare(r, c, h.value.clone());
            cell.origin = (h.src_row, h.src_col);
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
                let subs: &[crate::arena::CellId] = s.ext_cells_ref().unwrap_or(&[]);
                let mut src = SourceRef::new(
                    s.src_row,
                    s.src_col,
                    ExtendMode::from_code(s.extend).unwrap_or(ExtendMode::Default16),
                );
                src.row_merge = s.engine_row_merge;
                src.col_merge = s.engine_col_merge;
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
    let areas = crate::calc::merged_view::areas(ctx);
    (out, areas)
}

/// 计算态几何：把**模板**行高/列宽按扩展映射铺到计算态坐标上（分页器 M2-11 的输入）。
///
/// # 映射怎么来
///
/// 产品分页时读的是 `report.getRowCell(calcRow)` —— 扩展后的报表**已经**把模板行
/// `insertRows` 复制成计算态行，所以行高天然跟着走。本 crate 的 `ReportDefine` 不随扩展
/// 改写，故按扩展后轴头格的 [`HeadCell::src_row`] / [`HeadCell::src_col`]
/// （**模板出处**）反查模板。轴头 `(r,0)` / `(0,c)` 随扩展克隆，保留独立行列属性；
/// 正文合并延续槽共享锚点，不能用其出处推断所在行列的属性。
///
/// # 兜底
///
/// 计算态**尾部**可能多出整行为空的行（`engine_pager` 第 43 行、`engine_sum` 第 5~8 行、
/// `engine_rowheight` 第 15~17 行）—— 它们没有格，也就没有出处。兜底规则是
/// **沿用上一行的几何**（首行则用 `0.0` / `-91`）。这三个夹具的尾部行高与上一行相同，
/// 兜底结果与产品一致；但它**是推断**，不是移植（见 `docs/m2-engine-design.md` §15）。
///
/// # ⛔ 已声明的缺口
///
/// 先从模板几何按扩展出处铺开，再以 `calcProperties` 的运行时属性覆盖值替换内置项。
fn fill_geometry(ctx: &CalcCtx, rd: &ReportDefine, out: &mut CalcGrid) {
    let (row_size, col_size) = (ctx.grid.row_size as usize, ctx.grid.col_size as usize);
    let mut row_src: Vec<Option<i32>> = vec![None; row_size];
    let mut row_ty: Vec<Option<i8>> = vec![None; row_size];
    let mut col_src: Vec<Option<i32>> = vec![None; col_size];
    for r in 1..row_size {
        if let Some(&id) = ctx.grid.by_pos.get(&(r as i32, 0)) {
            let h = &ctx.grid.cells[id.0 as usize];
            row_src[r] = Some(h.src_row);
            row_ty[r] = Some(h.row_type);
        }
    }
    for (c, source) in col_src.iter_mut().enumerate().skip(1) {
        if let Some(&id) = ctx.grid.by_pos.get(&(0, c as i32)) {
            *source = Some(ctx.grid.cells[id.0 as usize].src_col);
        }
    }
    let mut last_h = 0.0f32;
    let mut last_t = -91i8;
    let mut last_row_visible = true;
    let mut last_row_break_page = false;
    let mut last_row_break_column = false;
    let mut last_row_adjust_height = false;
    for r in 1..row_size {
        let owner = ctx.grid.by_pos.get(&(r as i32, 0)).copied();
        let calculated =
            |key| owner.and_then(|id| crate::calc::properties::property_value(ctx, id, key));
        let row_index = row_src[r].map(|s| s as usize);
        let template = row_index.and_then(|s| rd.row_cells.get(s.saturating_sub(1)));
        let calculated_h = owner.and_then(|id| crate::calc::properties::float_property(ctx, id, 1));
        let h = calculated_h
            .or_else(|| row_src[r].and_then(|s| rd.row_height(s as usize)))
            .unwrap_or(last_h);
        let t = row_ty[r].unwrap_or(last_t);
        let row_visible = calculated(2)
            .and_then(|v| match v {
                Value::Bool(value) => Some(*value),
                _ => None,
            })
            .or_else(|| template.map(|rc| rc.row_visible))
            .unwrap_or(last_row_visible);
        let row_break_page = calculated(13)
            .and_then(|v| match v {
                Value::Bool(value) => Some(*value),
                _ => None,
            })
            .or_else(|| template.map(|rc| rc.break_page))
            .unwrap_or(last_row_break_page);
        let row_break_column = calculated(15)
            .and_then(|v| match v {
                Value::Bool(value) => Some(*value),
                _ => None,
            })
            .or_else(|| template.map(|rc| rc.break_column))
            .unwrap_or(last_row_break_column);
        let row_adjust_height = calculated(7)
            .and_then(|v| match v {
                Value::Bool(value) => Some(*value),
                _ => None,
            })
            .or_else(|| template.map(|rc| rc.adjust_row_h))
            .unwrap_or(last_row_adjust_height);
        out.row_heights.push(h);
        out.row_types.push(t);
        out.row_group_levels
            .push(template.map_or(1, |rc| rc.group_header_level));
        out.row_visible.push(row_visible);
        out.row_break_page.push(row_break_page);
        out.row_break_column.push(row_break_column);
        out.row_adjust_height.push(row_adjust_height);
        last_h = h;
        last_t = t;
        last_row_visible = row_visible;
        last_row_break_page = row_break_page;
        last_row_break_column = row_break_column;
        last_row_adjust_height = row_adjust_height;
    }
    let mut last_w = 0.0f32;
    let mut last_col_visible = true;
    let mut last_col_break_page = false;
    let mut last_col_break_column = false;
    // 跳过下标 0（列头轴），与行属性循环对称。
    for (index, s) in col_src.iter().copied().enumerate().skip(1) {
        let owner = ctx.grid.by_pos.get(&(0, index as i32)).copied();
        let calculated =
            |key| owner.and_then(|id| crate::calc::properties::property_value(ctx, id, key));
        let col_index = s.map(|source| source as usize);
        let template = col_index.and_then(|c| rd.col_cells.get(c.saturating_sub(1)));
        let calculated_w = calculated(11).and_then(|v| match v {
            Value::Float(n) => Some(*n),
            _ => None,
        });
        let w = calculated_w
            .or_else(|| s.and_then(|source| rd.col_width(source as usize)))
            .unwrap_or(last_w);
        let col_visible = calculated(12)
            .and_then(|v| match v {
                Value::Bool(value) => Some(*value),
                _ => None,
            })
            .or_else(|| template.map(|cc| cc.col_visible))
            .unwrap_or(last_col_visible);
        let col_break_page = calculated(13)
            .and_then(|v| match v {
                Value::Bool(value) => Some(*value),
                _ => None,
            })
            .or_else(|| template.map(|cc| cc.break_page))
            .unwrap_or(last_col_break_page);
        let col_break_column = calculated(15)
            .and_then(|v| match v {
                Value::Bool(value) => Some(*value),
                _ => None,
            })
            .or_else(|| template.map(|cc| cc.break_column))
            .unwrap_or(last_col_break_column);
        out.col_widths.push(w);
        out.col_types.push(template.map_or(-79, |cc| cc.col_type));
        out.col_visible.push(col_visible);
        out.col_break_page.push(col_break_page);
        out.col_break_column.push(col_break_column);
        last_w = w;
        last_col_visible = col_visible;
        last_col_break_page = col_break_page;
        last_col_break_column = col_break_column;
    }
}
