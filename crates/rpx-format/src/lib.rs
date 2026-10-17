//! JSON report definition → `ReportDefine`。
//!
//! 这个 crate 是 JSON 解析/构建的唯一入口：不依赖 `.rpx` 二进制读写，
//! 因而引擎测试、运行时和未来的 v2.0 默认构建可以共用同一套表。

pub mod definition;
pub mod reportdef;
