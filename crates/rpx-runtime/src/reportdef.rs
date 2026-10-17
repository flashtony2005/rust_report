//! Deprecated path: JSON report definition parsing now lives in `rpx-format`.
//!
//! 保留 `rpx_runtime::reportdef` 作为兼容 re-export；新代码请直接依赖
//! `rpx_format::reportdef`。

pub use rpx_format::reportdef::*;
