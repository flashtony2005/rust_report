//! head 图：找出并归一每格的左 head / 上 head（§12.1 第 5/6 步）。
//!
//! 算法正文：
//! - `find.rs` 的逐行契约 ⇒ `docs/format-contract.md` **附件 I**、**附件 J**；
//! - head 查找的伪码与「四处不对称」表 ⇒ `docs/m2-engine-design.md` **§4.2**；
//! - 归一化（把 head 链收到最近的 `ExtExtendCell`）⇒ `docs/m2-engine-design.md` **§4.4**；
//! - 回归基线（12 夹具逐格 head 真值）⇒ `tests/fixtures/head_matrix.txt` 与
//!   各夹具 `expected.json` 的 `left_head`/`top_head` 键。
//!
//! 本文件只放模块说明与公开面；查找在 [`find`]，生产格表在 [`grid`]（格属性在
//! [`cell`]），归一在 [`normalize`]。

mod cell;
mod find;
mod grid;
mod normalize;
mod runtime;

pub use cell::HeadCell;
pub use find::{find_left_head, find_top_head, HeadGrid, HeadResult};
pub use grid::HeadState;
pub use normalize::normalize_heads;
pub use runtime::CellRuntime;

#[cfg(test)]
mod runtime_tests;

#[cfg(test)]
mod tests;