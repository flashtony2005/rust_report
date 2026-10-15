//! CSV 导出（M6-01）—— 契约：`docs/spec-m6.md` §2.3（**自定** RFC 4180 子集）。
//!
//! # 为什么自定而不是复刻产品
//!
//! 产品（ReportLite）导出 CSV 的真实记法（空值记 `""` 还是 `null`、是否带表头、
//! 数字千分位）**没有 Java 真值可对拍**（spec-m6 §5.1 带账①）⇒ 本仓自定一套
//! 写死的规则，让导出可被单测与读回自洽对拍钉死。
//!
//! # 规则（逐条都有测试）
//!
//! 1. **满矩阵展开**：按 `(row, col)` 1..=rows × 1..=cols 展开；
//!    [`CalcGrid::cells`] 是**稀疏**的（只收 `is_emitted()` 为真的格），
//!    不在里面的格 ⇒ 空串，**不留洞**（`engine_sum` 的 8×9 网格就是 72 字段）；
//! 2. **取值**：`disp_value` 优先于 `value`（与前端展示一致，`engine_disp` 的
//!    `(2,3)`：`value=null`、`disp_value="仅显示值"` ⇒ 导出"仅显示值"）；
//!    `value` 为 null ⇒ 空串；
//! 3. **转义**：字段含 `,` `"` `\n` `\r` 任意之一 ⇒ 整体双引号包裹、内部 `"`
//!    双写（RFC 4180 子集）；
//! 4. **行尾** `\r\n`；**末尾不留换行**（join 语义——写进测试钉死）；
//! 5. **BOM 不写**：CSV 面向对拍与数据交换，不是给 Excel 双击的（Excel 中文
//!    乱码问题由 M6-02 的 xlsx 通道解决）。
//!
//! ⚠ 数字的文本口径 = 本仓 [`crate::text::value_text`]（Rust shortest
//! round-trip Display），与产品 `String.valueOf` **不保证逐字一致**——自定
//! 契约的一部分，不是遗漏。

use std::collections::HashMap;

use rpx_engine::arena::{CalcCell, CalcGrid};

use crate::text::cell_text;

/// 计算态网格 → CSV 文本（规则见模块文档）。
///
/// 顺序：行优先（r=1..=rows，每行内 c=1..=cols）。返回的字符串**不含**末尾
/// 换行；空网格（rows=0 或 cols=0）返回空串。
pub fn to_csv(grid: &CalcGrid) -> String {
    if grid.rows <= 0 || grid.cols <= 0 {
        return String::new();
    }
    // 稀疏格先建索引：往后的每一格都是 O(1) 且不依赖 cells 的排列顺序。
    let mut index: HashMap<(i32, i32), &CalcCell> = HashMap::with_capacity(grid.cells.len());
    for cell in &grid.cells {
        index.insert((cell.row, cell.col), cell);
    }

    let mut out = String::new();
    for r in 1..=grid.rows {
        if r > 1 {
            out.push_str("\r\n");
        }
        for c in 1..=grid.cols {
            if c > 1 {
                out.push(',');
            }
            let text = match index.get(&(r, c)) {
                Some(cell) => cell_text(cell),
                None => String::new(),
            };
            write_field(&mut out, &text);
        }
    }
    out
}

/// 按规则 3 写一个字段（`"` 双写 + 包裹）。
fn write_field(out: &mut String, s: &str) {
    if !needs_quotes(s) {
        out.push_str(s);
        return;
    }
    out.push('"');
    for ch in s.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
}

/// 是否必须包裹（RFC 4180：含分隔符/引号/换行的字段）。
fn needs_quotes(s: &str) -> bool {
    s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r')
}

#[cfg(test)]
mod tests;
