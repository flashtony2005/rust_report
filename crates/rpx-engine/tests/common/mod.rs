//! 测试共享支撑：把产品产出的 `tests/fixtures/engine_*.expected.json`
//! 载入为类型化结构（[`model::Fixture`]）。
//!
//! # 为什么单独抽一层
//!
//! `docs/spec-m2.md` §7 的验收项 M2-09 要求计算态**逐格**断言（值 + 类型 +
//! `source` 的 6 个子字段）、分页**逐页**断言、合并**逐格**断言 —— "只比矩阵维度
//! 不算通过"。这些 `.expected.json` 是产品引擎自己跑出来的唯一事实源，
//! 不是手写期望（`GenEngineFixtures.java:1-28`）。没有这一层，引擎就无法被验收。
//!
//! # 为什么放行 dead_code / unused_imports
//!
//! Rust 的 integration test 各自把本模块编译一份：某个 test 只用到其中一部分，
//! 而 `mod common;` 是**私有**模块，未用到的 `pub` 项在测试二进制里不可从根部到达，
//! 会被 `dead_code` 判为未使用，在 `-D warnings` 下直接失败。故在此统一放行。
//! 这只影响测试支撑代码，不掩盖被测代码（`src/`）的任何问题。
#![allow(dead_code, unused_imports)]

pub mod compare;
pub mod json;
pub mod loader;
pub mod model;

pub use compare::{
    compare_actual_text, compare_actual_with_raw, compare_fixture_to_itself, Diff, Dim, Kind,
    Report,
};
pub use loader::{all_names, load, load_raw, parse_fixture};
pub use model::*;
