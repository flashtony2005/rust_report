//! `rpx-export` —— 导出面 crate（M6-01 起）。
//!
//! 契约正文：`docs/spec-m6.md` §2（设计决策）/ §2.3（CSV 格式契约，**自定**，
//! 非产品格式——产品导出真值未探针，见 §5.1 带账）。
//!
//! 三条边界（spec-m6 §2.1 / §4）：
//!
//! 1. **输入是已经算完的网格** —— 吃 [`rpx_engine::arena::CalcGrid`]，不读
//!    `.rpx`、不跑 SQL、不重跑引擎（导出器与「重算」解耦）；
//! 2. **不碰零依赖层** —— `rpx-core`/`rpx-engine` 的 `[dependencies]` 仍只许
//!    path（G5 不变）；本 crate 自身的第三方依赖（M6-02 的 `rust_xlsxwriter`）
//!    必须先在 `scripts/dep_allowlist.txt` 登记并在 `code-review.md` §3.2 写明理由；
//! 3. **不做版式保真** —— 边框/字体/对齐/行高列宽/PDF 打印整体后置（spec-m6
//!    §5.2）；本 crate 只做**数据保真**（值 + 合并区）。
//!
//! 出口顺序（spec-m6 §2.2）：CSV（本文件+M6-01）→ xlsx（M6-02）→
//! server `/export`（M6-03）→ CLI `render --out`（M6-04）→ 夹具基线（M6-05）。

pub mod csv;
mod text;
pub mod xlsx;

pub use csv::to_csv;
pub use xlsx::to_xlsx;
