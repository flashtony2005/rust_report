//! 格地址格式化 —— 错误链与诊断共用（M5-09 因 G4 行数门禁从 `run.rs` 拆出）。
//!
//! 关键口径：错误链必须报**模板格号**（`h.src_row/src_col`），不是计算态地址——
//! 扩展会把模板 C2 挪到 D2，而产品报的仍是 C2（见 `spec-m5-09` §10、
//! `tests/fixtures/RUNLOG.md` 第三批）。

use super::ctx::CalcCtx;
use crate::arena::CellId;

/// 计算态格 → **模板**格号（如 `"C2"`）。
pub(super) fn src_addr_name(ctx: &CalcCtx, id: CellId) -> String {
    match ctx.grid.get(id) {
        Some(h) => ab_name(h.src_row, h.src_col),
        None => format!("cell#{}", id.0),
    }
}

/// `(row, col)` → `"B2"`（列字母 26 进制，A=1）。
pub(super) fn ab_name(row: i32, col: i32) -> String {
    let (r, mut c) = (row, col);
    let mut letters = Vec::new();
    while c > 0 {
        letters.push((b'A' + ((c - 1) % 26) as u8) as char);
        c = (c - 1) / 26;
    }
    letters.reverse();
    format!("{}{}", letters.into_iter().collect::<String>(), r)
}
