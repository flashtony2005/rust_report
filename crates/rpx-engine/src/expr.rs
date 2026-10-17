//! 表达式切片：词法 + Pratt 语法分析 + `is_extend()` 谓词（**不含求值**）。
//!
//! 算法正文（本项目约定：算法写进 md，代码只留指针）：
//! **`docs/m2-engine-design.md` §13**（13.1 token/优先级、13.2 AST 清单、
//! 13.3 `is_extend` 真值表、13.4 名字表、13.5 未支持清单）。
//!
//! # 为什么表达式切片必须先于扩展算法落地
//!
//! 阶段 B 的第 1 步 `checkExtend`（`ExtNormalCell.java:492-514`）在声明 `extendMode=16` 时
//! 会问一句：**这个表达式驱不驱动扩展**（`expression.isExtend()`）。答案决定该格会不会被
//! 包成 `ExtExtendCell`，进而决定拓扑改写与整张计算矩阵的行列结构。
//! 所以 `is_extend()` 是扩展算法（§12.1 第 3 步）的**前置依赖**，而不是"顺带的功能"。
//!
//! # 本切片做什么 / 不做什么
//!
//! | 做 | 依据 |
//! |----|------|
//! | 词法（`scanId` 规则） | `Expression.scanId:70-78` |
//! | Pratt 语法分析（绑定力 = 产品 `priority`） | `Expression.create:289-456` + 各运算符类 |
//! | AST 上的 `is_extend()`（纯递归谓词） | `Expression.isExtend:561-566` + 各节点 `isExtended()` |
//!
//! | ⛔ **不做** | 原因 |
//! |-----------|------|
//! | **求值**（`calculate`） | 已落地：[`eval`]（求值器）+ [`resolve`]（CSVariable 相对引用）+ [`ds`]（运行时数据集），见三者的模块文档 |
//! | `optimize()` 常量折叠 | 折叠会让 `sort(...)` 等变成 `Constant(List)`；对 `is_extend()` 的**结论无影响**（两者都真），但要让 `checkExtend` 的调用顺序完全对齐还需调度器 |
//! | 相对引用解析（`Move`/`Offset`/`DSMember` 的语义） | M2-06，需要 head 图 |
//! | `IParam` 嵌套树 | 只建模到顶层一层，见 §13.5 |
//!
//! # 两个反例（本项目实测踩过，别再踩）
//!
//! 1. ⛔ **不能用 `isDSFunction()` 近似 `isExtend()`**。两条链传播规则**相反**：
//!    `Function.isDSFunction()`（`Function.java:55-57`）向参数透传；`isExtended()` 不透传
//!    （`Function`/`Sum`/`DSSum` 都没覆写）。于是 `sum(ds1.select(qty))` 的
//!    `isDSFunction()=true` 而 `isExtended()=false` —— 这条是回归单测。
//! 2. ⛔ **不能用"名字像 DS 就当 DS 函数"**。`plot`/`sum`/`max`/`min`/`avg`/`count`
//!    在普通表与 DS 表里**都有登记**，裸调用时普通表优先（`Expression.java:219-224`），
//!    所以 `plot(x)` 是常规 `Plot`（false），只有 `ds1.plot(x)` 才是 `DSPlot`（true）。
//!    且 `isExtend()` 那条路径上 `ds` **恒为 null**（`ExtNormalCell.java:498-504`），
//!    裸 `select(qty)` 根本走不到 DS 回退。详见 [`names`] 的模块文档。

pub mod ast;
pub mod atom;
pub mod call;
pub mod cellset;
pub mod classify;
pub mod coord;
pub mod ds;
pub mod dsfn;
pub mod dsgroup;
pub mod dsselect;
pub mod dsstate;
pub mod eval;
pub mod lexer;
pub mod names;
pub mod num;
pub mod parser;
pub mod resolve;
pub mod token;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_aggregate;
#[cfg(test)]
mod tests_coord;
#[cfg(test)]
mod tests_eval;
#[cfg(test)]
mod tests_offset;
#[cfg(test)]
mod tests_select;

pub use ast::{BinOp, Call, Expr, Param, ParamKind, UnOp};
pub use classify::err;
pub use ds::{DataSet, DsCur};
pub use eval::{evaluate, EvalCtx};
pub use lexer::lex;
pub use names::{classify_bare, classify_ds, CallKind, DS, DS_EXTEND, REGULAR, REGULAR_EXTEND};
pub use parser::{parse, parse_tokens_with, parse_with};
pub use token::Tok;

use crate::error::Result;
use crate::params::ParamTable;

/// 解析一条表达式源码。
pub fn parse_str(src: &str) -> Result<Expr> {
    parse(src)
}

/// 一步到位：解析 + `is_extend()`。这是扩展算法（§12.1 第 3 步）需要的那个入口。
///
/// 解析失败会 `Err`（产品侧同样是抛 `ReportError`，`Expression.java:489-493`），
/// **不**静默返回 `false` —— "因为错的原因得出对的答案"是本项目明确要避免的失败模式。
///
/// `params` **强制显式传入**（M3-01）：`is_extend` 是准备期判定，参数值必须在这一步
/// 可得（`@p` 未登记 ⇒ [`crate::error::EngineError::UnknownParam`]）。不提供"无表"
/// 便利版 —— 漏传参数表会静默把该扩展的格判成不扩展，这是会算错的静默分叉。
pub fn is_extend(src: &str, params: &ParamTable) -> Result<bool> {
    Ok(parse_with(src, params)?.is_extend())
}
