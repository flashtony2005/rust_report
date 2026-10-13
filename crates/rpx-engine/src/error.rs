//! 引擎错误。
//!
//! 与 Java 侧的对应物：
//! - `CircularRef` ← `com.raqsoft.commonl.ReportError`，其 cause 是诊断器
//!   `com.raqsoft.reportl.model.engine.IIllIIIIIIIIIlll`；该诊断器 message 里带
//!   **引用链字符串**（实测 `B2-C2-B2-C2` / `B2-B2-B2` / `C2-B2-C2`），
//!   所以这里保留 `chain` 而不是照抄本地化文案 —— 文案会随语言包变，链不会。
//! - `UnknownCell` ← `Expression.unknownCell`（注意 `$B$2` 也走这条：`$` 只剥一个，
//!   剩下的 `B$2` 解析失败）。
//!
//! 契约：成环必须**返回错误**，不得 panic、不得死循环（M2-07）。

use std::fmt;

/// 引擎错误。全部是"可预期的失败"，不用于表示编程错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// 表达式引用成环。`chain` 是用 `-` 连接的格号序列，如 `B2-C2-B2-C2`。
    CircularRef { chain: String },
    /// 引用了不存在的格号（含 Excel 式 `$B$2` 这种产品不支持的写法）。
    UnknownCell { id: String },
    /// 语法或函数面暂不支持。
    UnsupportedExpr { expr: String },
    /// 表达式求值出的格号越界。
    OutOfRange { row: i32, col: i32, rows: i32, cols: i32 },
    /// 报表声明了数据集但运行时没注入。
    MissingDataset { name: String },
    /// 计算被外部中断（Java 的 `_$8()`：`_$11` 标志为真时抛
    /// `ReportError("Calculation is interrupted!")`，`ExtCellSet.java:233-237`）。
    Interrupted,
    /// special 队列重入超上限。产品对 state 6 的重入**没有上限会活锁**
    /// （`ExtCellSet.java:1438-1441` 重新入队），Rust 侧必须显式设限报错，
    /// 不得静默死循环（`format-contract.md` 附件 K.1 硬约束 ②）。
    SpecialQueueLivelock { requeues: usize },
    /// 列扩展前置校验失败：锚点所在行不是"行头行"（`ExtCellSet.java:1086-1090`
    /// 抛 `ReportError`，消息键 **`ExtCellSet.entendCol`** —— 附件 I.6.1：
    /// 产品把 extend 拼错成 entend，**逐字保留，别纠正**）。
    /// 目前无夹具覆盖（附件 I.10 建议补）。
    EntendCol { row: i32 },
    /// 产品源码里的"不可能状态"（Java 侧会 NPE/越界）：Rust 侧显式报错、
    /// 不 panic（附件 I.10 的同类纪律）。`detail` 写明出处与状态。
    Invariant { detail: String },
    /// 调度挂起穿过求值器（Java 的 `IIllIIIIIIIIIlll` 是 `Throwable`，能从
    /// `Expression.calculate` 深处一路穿透到通道；Rust 的 [`crate::expr::eval::evaluate`]
    /// 错误类型是 `EngineError`，挂起必须**搭载**在它上面才能穿过）。
    ///
    /// ⛔ 只在求值链内部短暂存在：`cell_calculate` 的出口会把它还原成
    /// [`Flow::Suspend`]（`calc/cell_eval.rs`），通道与 `flow_to_result`
    /// 永远不该见到它 —— 见到即移植层 bug。
    Suspend(crate::eval::Suspend),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::CircularRef { chain } => {
                write!(f, "circular reference: {chain}")
            }
            EngineError::UnknownCell { id } => write!(f, "unknown cell: {id}"),
            EngineError::UnsupportedExpr { expr } => {
                write!(f, "unsupported expression: {expr}")
            }
            EngineError::OutOfRange { row, col, rows, cols } => {
                write!(f, "cell ({row},{col}) out of range {rows}x{cols}")
            }
            EngineError::MissingDataset { name } => {
                write!(f, "missing dataset: {name}")
            }
            EngineError::Interrupted => write!(f, "calculation is interrupted"),
            EngineError::SpecialQueueLivelock { requeues } => {
                write!(f, "special queue requeued {requeues} times without progress")
            }
            EngineError::EntendCol { row } => {
                write!(f, "ExtCellSet.entendCol: row {row} is not a row-header row")
            }
            EngineError::Invariant { detail } => write!(f, "engine invariant broken: {detail}"),
            EngineError::Suspend(s) => write!(f, "suspend (state {})", s.state),
        }
    }
}

impl std::error::Error for EngineError {}

/// 引擎侧结果别名。
pub type Result<T> = std::result::Result<T, EngineError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_ref_keeps_the_chain_not_the_localised_text() {
        let e = EngineError::CircularRef { chain: "B2-C2-B2-C2".into() };
        assert_eq!(e.to_string(), "circular reference: B2-C2-B2-C2");
    }

    #[test]
    fn errors_are_comparable_so_callers_can_assert_on_them() {
        let a = EngineError::UnknownCell { id: "$B$2".into() };
        let b = EngineError::UnknownCell { id: "$B$2".into() };
        assert_eq!(a, b);
    }
}
