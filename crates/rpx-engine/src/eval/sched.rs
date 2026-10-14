//! 三条调度通道（`ExtCellSet.java:1297-1446` 的逐行移植）与统一入口。
//!
//! # 移植对照表
//!
//! | Rust | Java | 筛选 | 消化的 state |
//! |------|------|------|------------|
//! | [`channel4`] | `_$1(ctx, st):1297-1364` | `extensible==19` 一趟、`==18` 一趟 | 4 |
//! | [`channel5`] | `_$2(ctx, special, st):1366-1426` | `extensible==17` | 4/5 换格、6 入队、8 级联失效 |
//! | [`channel6`] | `_$1(ctx, special, st):1428-1446` | special 队列逐个 | 6（重入有上限） |
//!
//! 三处**必须照抄**的细节：
//! 1. 位置同一性检查 `row(cell)!=i || col(cell)!=j ⇒ 跳过`（`:1308` 等）——
//!    扩展后矩阵槽位上可能坐着不属于该槽的格；
//! 2. state 6 时 `special.push(**当前格**)` 而不是挂起方 `c`（`:1400`），
//!    且 `c.reset()`、**不排空 st**；
//! 3. state 8 只对 `getSource()==self` 的**本体格**标 8 入队，克隆跳过
//!    （`:1406-1416`）——失效标记随队列去重。

use std::collections::VecDeque;

use super::{state, Flow};
use crate::arena::CellId;
use crate::error::EngineError;

/// special 队列重入上限（超过即判活锁）。产品无上限；这里取一个远大于
/// 任何合法重排次数、又远小于死循环的值。合法场景：n 个格互相等 special
/// 通道，最多 n² 次重排必然收敛或成环。
pub const DEFAULT_MAX_SPECIAL_REQUEUES: usize = 100_000;

/// 调度器眼中的世界（对应 Java 的 `this`，即 `ExtCellSet` 的可观察面）。
///
/// 与 [`crate::heads::HeadGrid`] 同一设计立场：全部以 `CellId` 寻址、
/// 不暴露内部布局，测试可注入脚本化假实现。
pub trait SchedGrid {
    /// 计算矩阵尺寸（**含** 0 号行/列，与 `_$16`/`_$15` 同口径）。
    fn dims(&self) -> (i32, i32);
    /// 计算矩阵 (row, col) 的格；越界/空槽 ⇒ [`CellId::NONE`]。
    fn get_calc(&self, row: i32, col: i32) -> CellId;
    fn row(&self, id: CellId) -> i32;
    fn col(&self, id: CellId) -> i32;
    /// 生效 extensible（阶段 B 改写后的 16..19）。
    fn extensible(&self, id: CellId) -> i8;
    /// `getSource() == this`：是否引擎矩阵本体格（克隆实例为 false）。
    fn is_original(&self, id: CellId) -> bool;
    /// `cell.calculate(ctx)`：`Ok(())` 完成；`Err(Flow::Suspend)` 请求换格；
    /// `Err(Flow::Fatal)` 是**终局错误**（求值失败 / 循环引用 / 中断）。
    ///
    /// ⛔ 为什么返回 [`Flow`] 而不是 [`Suspend`]：Java 的 `calculate` 抛的是
    /// `Throwable`，产品拿**同一个异常类型**承载「调度挂起」与「真实错误」；
    /// 通道里的 `catch (IIllIIIIIIIIIlll)` 只接调度挂起，其余异常自然穿透上抛
    /// （`:1319-1329`）。Rust 侧若只用 `Suspend`，[`crate::expr::eval`] 抛出的
    /// [`crate::error::EngineError`] 就**无处可去** —— 只能被压成"完成"，
    /// 于是真错误静默变成一个空值。这条签名是被
    /// `engine_cycle`（错误型夹具）逼出来的，不是为了好看。
    fn calculate(&mut self, id: CellId) -> Result<(), Flow>;
    /// 每格求值前的钩子：`_$21 = cell; _$19.setCell(cell, cell00)`。
    fn prepare_eval(&mut self, id: CellId);
    /// `_$8()`：中断检查。真 ⇒ 通道立即以 [`Flow::Fatal`] 退出。
    fn interrupted(&self) -> bool;
    /// `cell.reset()`：清计算标志与缓存值。
    fn reset(&mut self, id: CellId);
    /// `setCalState(8)`（state 8 级联失效标记）。
    fn set_cal_state(&mut self, id: CellId, st: u8);
    /// `_$24.indexOf(c) >= 0`（失效队列去重查询）。
    fn fail_queued(&self, id: CellId) -> bool;
    /// `_$24.push(c)`。
    fn fail_queue_push(&mut self, id: CellId);
    /// 引擎级轮次 `_$14`（观测用；测试断言通道切换顺序）。
    fn set_engine_state(&mut self, st: u8);
}

/// 同一性检查（`:1308`/`:1338`/`:1374`）：槽位 (i,j) 上的格必须自称就在 (i,j)。
fn slot_matches<G: SchedGrid>(g: &G, cell: CellId, i: i32, j: i32) -> bool {
    g.row(cell) == i && g.col(cell) == j
}

/// 通道 4 —— 横向扩展格（19）先跑一趟、纵向扩展格（18）再跑一趟。
///
/// 只消化 state 4（FIRST）：挂起方压栈、跳到需求值方重试；其余态连当前格
/// 一起压栈后原样上抛（`:1322-1328`）。
///
/// ⛔ **循环边界必须每轮重读**：Java 写的是 `for (i = 1; i < this._$16; ++i)`
/// （`:1304`/`:1334`），而 `_$16`/`_$15` 是**会被扩展改写的缓存**
/// （插入行/列时同步 `+=`，见 `docs/m2-engine-design.md` §12.1 的 U9）。
/// 于是上一格 `calculate` 里发生的 `extendRow/extendCol` 会**让本循环多跑几行/几列**。
/// 把 `dims()` 在循环外缓存（Rust 的 `for i in 1..rows` 天然如此）就**漏扫新插入的行列**，
/// 后果是「扩展出来的整段空白」—— 最贵的一类错。
pub fn channel4<G: SchedGrid>(g: &mut G, st: &mut Vec<CellId>) -> Result<(), Flow> {
    g.set_engine_state(state::FIRST);
    for target in [19i8, 18] {
        let mut i = 1;
        while i < g.dims().0 {
            let mut j = 1;
            while j < g.dims().1 {
                if g.interrupted() {
                    return Err(Flow::Fatal(EngineError::Interrupted));
                }
                let cell0 = g.get_calc(i, j);
                if !cell0.is_none() && g.extensible(cell0) == target && slot_matches(g, cell0, i, j)
                {
                    // chase 循环：对应 Java 的 try{while(true){..}}catch(…)
                    let mut cell = cell0;
                    loop {
                        g.prepare_eval(cell);
                        match g.calculate(cell) {
                            Ok(()) => match st.pop() {
                                Some(prev) => cell = prev, // 依赖链回卷
                                None => break,             // = Java `continue block5`
                            },
                            Err(Flow::Suspend(e)) if e.state == state::FIRST => {
                                st.push(cell);
                                cell = e.cell;
                            }
                            // 未处理挂起与终局错误都要把当前格压栈后原样上抛（`:1327-1328`）。
                            Err(flow) => {
                                st.push(cell);
                                return Err(flow);
                            }
                        }
                    }
                }
                j += 1;
            }
            i += 1;
        }
    }
    Ok(())
}

/// 通道 5 —— 表达式格（17）。
///
/// - state 4/5：与通道 4 同样换格；
/// - state 6：`c.reset()` 后把**当前格**压入 special 队列（`:1398-1401`）；
/// - state 8：**级联失效**——当前格标 8、挂起方 reset、本体格标 8 入 `_$24`、
///   然后把整个依赖栈排空逐一处理（`:1403-1417`）；
/// - 其余态上抛。
///
/// ⛔ 循环边界同 [`channel4`]：**每轮重读** `_$16`/`_$15`（`:1369`/`:1371`），
/// 因为本趟里发生的扩展会当场把缓存改大。
pub fn channel5<G: SchedGrid>(
    g: &mut G,
    special: &mut VecDeque<CellId>,
    st: &mut Vec<CellId>,
) -> Result<(), Flow> {
    g.set_engine_state(state::SECOND);
    st.clear(); // `st.reset()`（:1368）
    let mut i = 1;
    while i < g.dims().0 {
        let mut j = 1;
        while j < g.dims().1 {
            if g.interrupted() {
                return Err(Flow::Fatal(EngineError::Interrupted));
            }
            let cell0 = g.get_calc(i, j);
            if !cell0.is_none() && g.extensible(cell0) == 17 && slot_matches(g, cell0, i, j) {
                // ⚠ `:1381-1384` 的 key-40 宏分支依赖 `initial` 标志，反编译输出里
                // 该标志没有任何置真点 ⇒ 疑似死分支，未移植（见模块文档）。
                let mut cell = cell0;
                loop {
                    g.prepare_eval(cell);
                    match g.calculate(cell) {
                        Ok(()) => match st.pop() {
                            Some(prev) => cell = prev,
                            None => break,
                        },
                        Err(Flow::Suspend(e)) => match e.state {
                            state::FIRST | state::SECOND => {
                                st.push(cell);
                                cell = e.cell;
                            }
                            state::SPECIAL => {
                                g.reset(e.cell);
                                special.push_back(cell); // ⛔ 是当前格，不是挂起方
                                break; // `continue block3`：st **不**排空
                            }
                            state::THIRD => {
                                g.set_cal_state(cell, state::THIRD);
                                g.reset(e.cell);
                                if g.is_original(e.cell) && !g.fail_queued(e.cell) {
                                    g.set_cal_state(e.cell, state::THIRD);
                                    g.fail_queue_push(e.cell);
                                }
                                while let Some(p) = st.pop() {
                                    g.reset(p);
                                    if !g.is_original(p) || g.fail_queued(p) {
                                        continue;
                                    }
                                    g.set_cal_state(p, state::THIRD);
                                    g.fail_queue_push(p);
                                }
                                break;
                            }
                            _ => {
                                st.push(cell);
                                return Err(Flow::Suspend(e));
                            }
                        },
                        // 终局错误（求值失败等）同样压栈后穿透，不试图换格重试。
                        Err(flow) => {
                            st.push(cell);
                            return Err(flow);
                        }
                    }
                }
            }
            j += 1;
        }
        i += 1;
    }
    Ok(())
}

/// 通道 6 —— special 队列排空。
///
/// state 6 重新入队（`:1438-1441`）；产品无上限会活锁，这里以
/// [`DEFAULT_MAX_SPECIAL_REQUEUES`] 兜底报 [`EngineError::SpecialQueueLivelock`]。
pub fn channel6<G: SchedGrid>(
    g: &mut G,
    special: &mut VecDeque<CellId>,
    st: &mut Vec<CellId>,
    max_requeues: usize,
) -> Result<(), Flow> {
    g.set_engine_state(state::SPECIAL);
    let mut requeues = 0usize;
    while let Some(cell) = special.pop_front() {
        if g.interrupted() {
            return Err(Flow::Fatal(EngineError::Interrupted));
        }
        g.prepare_eval(cell);
        match g.calculate(cell) {
            Ok(()) => {}
            Err(Flow::Suspend(e)) if e.state == state::SPECIAL => {
                requeues += 1;
                if requeues > max_requeues {
                    return Err(Flow::Fatal(EngineError::SpecialQueueLivelock { requeues }));
                }
                special.push_back(cell);
            }
            Err(flow) => {
                st.push(cell);
                return Err(flow);
            }
        }
    }
    Ok(())
}

/// 统一入口：4 → 5 → 6，边界把未处理挂起转成 [`EngineError`]。
///
/// ⚠ 这是独立的值通道入口，不含顶层阶段 B、属性表达式、样式宏与尾列裁剪；生产入口
/// [`crate::calc::calculate`] 已把可建模的顶层步骤按顺序接齐。
pub fn run_channels<G: SchedGrid>(g: &mut G) -> crate::error::Result<()> {
    let mut st = Vec::new();
    let mut special = VecDeque::new();
    super::finish_channel(channel4(g, &mut st))?;
    super::finish_channel(channel5(g, &mut special, &mut st))?;
    super::finish_channel(channel6(
        g,
        &mut special,
        &mut st,
        DEFAULT_MAX_SPECIAL_REQUEUES,
    ))?;
    g.set_engine_state(state::FINISHED);
    Ok(())
}
