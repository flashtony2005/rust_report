//! 模板分页输入校验：拒绝未知页方式、无效栏数和未知行列类型。
//! 支持的行为与 JSON 名称见 docs/report-def-json.md 的教程打印契约。

use crate::error::{EngineError, Result};
use rpx_core::print_setup::PrintSetup;
use rpx_core::ReportDefine;

/// `IColCell.TYPE_NORMAL`（`IColCell.java:16`）：数据列。区带列是
/// `TYPE_LEFT_HEADER = -80` / `TYPE_RIGHT_HEADER = -78`（`:15`/`:17`）。
const COL_TYPE_DATA: i8 = -79;

/// 输入校验先于扩展计算，也由公开的模板分页入口复核。
pub fn ensure_supported(setup: &PrintSetup) -> Result<()> {
    if !(0..=2).contains(&setup.pager_style)
        || (setup.pager_style == 1 && setup.row_num_per_page <= 0)
    {
        return Err(EngineError::UnsupportedPaging {
            detail: format!(
                "pagerStyle={}（应为 0=不分页 / 1=按行数 / 2=按纸分页，按行数时每页行数必须为正）",
                setup.pager_style
            ),
        });
    }
    if setup.column_mode != 0 {
        return Err(EngineError::UnsupportedPaging {
            detail: format!(
                "columnMode={}（仅支持从上到下、从左到右的 columnMode=0）",
                setup.column_mode
            ),
        });
    }
    // `PagerInfo(PrintSetup)` 把 tableColumnNum 原带进 SimplePager（`PageBuilder.init1`
    // → `pager.setColumns(pi.getTableColumnNum())`，`PageBuilder.java:207`）；
    // 探针路径覆盖成 1（`CalcJson.renderPages` 实参），模板真值路径不会。
    if !(1..=16).contains(&setup.table_column_num) {
        return Err(EngineError::UnsupportedPaging {
            detail: format!(
                "tableColumnNum={}（分栏数应在 1..=16）",
                setup.table_column_num
            ),
        });
    }
    Ok(())
}

/// 左右列头与数据列均有行为；拒绝没有对应语义的其他类型码。
pub fn ensure_data_columns(rd: &ReportDefine) -> Result<()> {
    for (i, cc) in rd.col_cells.iter().enumerate() {
        if ![-80, COL_TYPE_DATA, -78].contains(&cc.col_type) {
            return Err(EngineError::UnsupportedPaging {
                detail: format!(
                    "第 {} 列 colType={}（应为 -80 左表头 / -79 数据列 / -78 右表头）",
                    i + 1,
                    cc.col_type
                ),
            });
        }
    }
    Ok(())
}
