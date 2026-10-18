pub mod capabilities;
pub mod catalog;
mod data;
pub mod datasource;
pub mod definition;
pub mod error;
pub mod execution;
pub mod files;
pub mod form;
pub mod format;
pub mod gallery;
#[cfg(feature = "legacy-rpx")]
pub mod import;
pub mod library;
pub mod lint;
/// 数字格式子集 → 显示文本。**实现在 `rpx-engine`**：导出面（`rpx-export`）
/// 算列宽时要用同一套口径，而它按分层矩阵不能依赖本 crate。
pub use rpx_engine::numfmt;
pub mod parameters;
pub mod pdf;
pub mod print_html;
pub mod reportdef;
pub mod result;
pub mod run;
pub mod snapshot;
