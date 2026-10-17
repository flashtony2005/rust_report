//! 浏览冻结使用计算态（分页时为页内）连续表头前缀，不改变数据或合并区。
use rpx_engine::arena::CalcGrid;
use serde_json::{json, Value};

/// Univer `IFreeze`：split 是行列数量，start 是第一个可滚动的 0 起索引。
/// 没有冻结的轴必须使用 `0/-1`；隐藏行列仍占索引，不能过滤后计数。
pub(super) fn for_grid(grid: &CalcGrid) -> Value {
    let rows = grid
        .row_types
        .iter()
        .take(grid.rows.max(0) as usize)
        .take_while(|t| matches!(t, -93 | -96 | -95))
        .count() as i32;
    let cols = grid
        .col_types
        .iter()
        .take(grid.cols.max(0) as usize)
        .take_while(|t| **t == -80)
        .count() as i32;
    json!({
        "xSplit": cols,
        "ySplit": rows,
        "startRow": if rows == 0 { -1 } else { rows },
        "startColumn": if cols == 0 { -1 } else { cols },
    })
}

#[cfg(test)]
mod tests;
