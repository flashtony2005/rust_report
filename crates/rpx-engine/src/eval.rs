//! 求值调度（§12.1 第 7 步）：把 Java 的「抛异常当 switch」协程翻译成
//! Rust 的显式 trampoline 状态机。
//!
//! 算法正文：`docs/format-contract.md` **附件 K.1**（四条通道 / 抛出点 /
//! bit30·bit31 位语义）；Java 源码 `ExtCellSet.java:1297-1446`。
//!
//! # 核心翻译规则（⛔ 硬约束，逐条有出处）
//!
//! 1. **禁止递归求值**：Java 用 `(cell, state)` 异常换格续跑；Rust 侧每条通道是
//!    一个显式 `loop`，挂起用 `Err(Suspend { cell, state })` 表达（K.1 要求 ①）。
//!    递归栈溢出且无法复现 state 8 的级联失效。
//! 2. **依赖栈 `st` 是 `Vec<CellId>`**：Java 的 `Stack(256)` 是软上限不是硬界。
//! 3. **state 6 会活锁**：special 队列的格再抛 6 就重新入队（`:1438-1441`），
//!    产品无上限；这里加了 [`DEFAULT_MAX_SPECIAL_REQUEUES`] 上限，超限报
//!    [`EngineError::SpecialQueueLivelock`]（K.1 要求 ②）。
//! 4. **state 1（CIRCLE）/ 3（PROPERTY）映射为错误**：等价产品的
//!    `CircularRefException`（M2-07）；在通道边界由 [`flow_to_result`] 转换。
//! 5. **bit30/bit31 永不出现在对拍里**：`rowMerge & 0x7FFFFFFF` 序列化时抹掉
//!    （`:2579`）——期望值不要包含这两位（K.1 要求 ③）。
//!
//! # 与顶层驱动的差距（诚实声明）
//!
//! 本模块提供三条通道（`channel4`/`channel5`/`channel6`）与统一入口
//! [`run_channels`]。顶层 `calculate()` 已接入阶段 B、值通道、M2-22 内置属性表达式
//! pass 与尾列裁剪；样式宏、外部属性对象仍未实现。单独的 [`run_channels`] 只是
//! 值通道调度器，不等同完整的 M2-20 顶层编排。
//!
//! 另：`_$2` 里 key-40 宏分支（`:1381-1384`）依赖的 `initial` 标志在反编译
//! 输出里**没有任何置真点** ⇒ 疑似死分支，未移植；若真机探针证伪此判断再补。

use crate::arena::CellId;
use crate::error::EngineError;

pub mod sched;

#[cfg(test)]
mod tests;

pub use sched::{
    channel4, channel5, channel6, run_channels, SchedGrid, DEFAULT_MAX_SPECIAL_REQUEUES,
};

/// 求值状态常量（`Node.java:45-53`）。
///
/// **1/3 是错误态**（CIRCLE / PROPERTY），**4/5/6/8 是调度态**；
/// 0/2/7 是生命周期标记（未算 / 解析中 / 完成）。
pub mod state {
    pub const NONE: u8 = 0;
    pub const CIRCLE: u8 = 1;
    pub const PARSE: u8 = 2;
    pub const PROPERTY: u8 = 3;
    pub const FIRST: u8 = 4;
    pub const SECOND: u8 = 5;
    pub const SPECIAL: u8 = 6;
    pub const FINISHED: u8 = 7;
    pub const THIRD: u8 = 8;
}

/// 挂起请求：Java `IIllIIIIIIIIIlll(cell, state)` 异常的 Rust 形态。
///
/// `cell` 是**需求值方**（想插队的格），`state` 是它要求调度器进入的通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Suspend {
    pub cell: CellId,
    pub state: u8,
}

impl Suspend {
    /// 构造挂起请求。
    pub fn new(cell: CellId, state: u8) -> Self {
        Suspend { cell, state }
    }
}

/// 通道内部的控制流：要么继续换格 chase，要么是**终局错误**。
///
/// 通道函数内部对「可处理的调度态」消化掉，把「未处理态」包成
/// [`Flow::Suspend`] 一路上抛；顶层（[`run_channels`]）统一转错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flow {
    /// 未被本通道处理的挂起 —— 按产品语义继续向外抛。
    Suspend(Suspend),
    /// 终局错误（中断 / 活锁等），不再重试。
    Fatal(EngineError),
}

impl From<Suspend> for Flow {
    fn from(s: Suspend) -> Self {
        Flow::Suspend(s)
    }
}

impl From<EngineError> for Flow {
    fn from(e: EngineError) -> Self {
        Flow::Fatal(e)
    }
}

/// 通道边界：把未处理挂起映射为 [`EngineError`]。
///
/// 定案口径（K.1 要求 ④）：
///
/// - state 1（CIRCLE）→ [`EngineError::CircularRef`]：产品在此抛
///   `CircularRefException`（M2-07）；`chain` 这里只有单格信息，
///   完整链由诊断器（`_$3`/`_$2` 的 :641-646/:678-683）另行产生。
/// - state 3（PROPERTY）→ 同样映射为 `CircularRef`：K.1 把 1/3 归为同一类
///   错误态（属性循环与值循环在产品里共用一个异常）。
/// - **4/5/6/8 已被对应通道消化，不可能到达这里**——为防御调度器自身缺陷，
///   一律报 `UnsupportedExpr` 携带状态与格号（断言式错误，见即知是移植层
///   出了 bug，而不是数据问题）。
pub fn flow_to_result(flow: Flow) -> crate::error::Result<()> {
    match flow {
        Flow::Fatal(e) => Err(e),
        Flow::Suspend(s) => match s.state {
            state::CIRCLE | state::PROPERTY => Err(EngineError::CircularRef {
                chain: format!("cell#{}", s.cell.0),
            }),
            other => Err(EngineError::UnsupportedExpr {
                expr: format!("未处理的调度状态 {other}（cell #{})", s.cell.0),
            }),
        },
    }
}

/// 通道收尾：`Ok(())` 直通；`Err(Flow)` 交给 [`flow_to_result`]。
pub fn finish_channel(r: Result<(), Flow>) -> crate::error::Result<()> {
    match r {
        Ok(()) => Ok(()),
        Err(flow) => flow_to_result(flow),
    }
}
