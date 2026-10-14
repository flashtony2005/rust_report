//! 分页配置的**支持性守卫**（`ensure_supported` 等）——2026-10-14 拆自 `pager.rs`
//! （行数门禁 G4；调用点在 `calc::calculate()` 入口）。
//!
//! [`super::paginate`] 按 `SimplePager._$5` 的 `pagerStyle == 2 && columns == 1`
//! 分支移植（探针口径，见 [`super::PAPER_W_PT`]）。以下 `PrintSetup` 字段会让
//! 产品的分页**改走别的分支**，而本实现不读它们 ⇒ 若静默按 `2` 跑，会产出**错误
//! 的分页**。故显式报 [`EngineError::UnsupportedPaging`]（`spec-m2.md` §6.2 带账项；
//! M3-06 接线时按 `spec-m3.md` §8 补齐列区带/换行两项）。
//!
//! ⛔ 这是"未实现"，**不是产品行为** —— 产品支持这些配置；它们带账到有产品探针 +
//! 夹具时实现（或 M3 的真实口径接线时处理，见 `spec-m3.md` §3.2）。

use crate::error::{EngineError, Result};
use rpx_core::print_setup::PrintSetup;
use rpx_core::ReportDefine;

/// `IColCell.TYPE_NORMAL`（`IColCell.java:16`）：数据列。区带列是
/// `TYPE_LEFT_HEADER = -80` / `TYPE_RIGHT_HEADER = -78`（`:15`/`:17`）。
const COL_TYPE_DATA: i8 = -79;

/// 分页配置支持性校验：只覆盖 `pagerStyle==2 && columns==1` 单栏分支。
///
/// 非默认配置返回 `Err(UnsupportedPaging)`；默认配置返回 `Ok(())`。
/// 调用点：`calc::calculate()` 入口（输入校验失败不跑计算）。
pub fn ensure_supported(setup: &PrintSetup) -> Result<()> {
    if setup.pager_style != 2 {
        return Err(EngineError::UnsupportedPaging {
            detail: format!(
                "pagerStyle={}（本实现只覆盖 2=按页高断页；0=单页短路 / 1=按行数断页未实现）",
                setup.pager_style
            ),
        });
    }
    if setup.column_mode != 0 {
        return Err(EngineError::UnsupportedPaging {
            detail: format!("columnMode={}（多栏布局未实现）", setup.column_mode),
        });
    }
    if setup.column_repeated_group_header {
        return Err(EngineError::UnsupportedPaging {
            detail: "isColumnRepeatedGroupHeader=true（多栏重复组头未实现）".to_string(),
        });
    }
    // `PagerInfo(PrintSetup)` 把 tableColumnNum 原带进 SimplePager（`PageBuilder.init1`
    // → `pager.setColumns(pi.getTableColumnNum())`，`PageBuilder.java:207`）；
    // 探针路径覆盖成 1（`CalcJson.renderPages` 实参），模板真值路径不会。
    if setup.table_column_num != 1 {
        return Err(EngineError::UnsupportedPaging {
            detail: format!(
                "tableColumnNum={}（多栏未实现：split_rows 只移植 columns==1 分支）",
                setup.table_column_num
            ),
        });
    }
    Ok(())
}

/// 列区带守卫（M3-06 补，`spec-m3.md` §8「列区带/换行守卫」项）。
///
/// 产品的 `_$2(r)` 列范围是「首个到末个**非区带**列」（`SimplePager.java:1318-1346`），
/// 而本实现的 [`super::split_cols`] 固定按 `1..=cols`（全列当数据列）断页。
/// 模板里出现区带列（`colType` 为 `-80` 左列头 / `-78` 右列头）时两者不等价
/// ⇒ 显式报错，不静默按全列断页。
pub fn ensure_data_columns(rd: &ReportDefine) -> Result<()> {
    for (i, cc) in rd.col_cells.iter().enumerate() {
        if cc.col_type != COL_TYPE_DATA {
            return Err(EngineError::UnsupportedPaging {
                detail: format!(
                    "第 {} 列 colType={}（列区带未实现：split_cols 固定按 1..=cols 数据列断页；\
                     产品 _$2(r) 的范围会排除区带列）",
                    i + 1,
                    cc.col_type
                ),
            });
        }
    }
    Ok(())
}

// ⛔ **换行回填没有静态守卫，也不该有**（2026-10-14 核对结论，M3-06）：
// `SimplePager._$1:1557-1573` 的测量分支条件是
// `cellType ∈ {-64, -56} && columns==1 && value instanceof String && 行类型 != -90`，
// 而 **`-64` 就是 `INormalCell.TYPE_NORMAL`**（普通文本格的默认类型，任何模板
// 都有大量）——静态期既不知道求值后是不是 String，也不知道页内放不放得下
// （那正是断页时才判定的事）。按"存在 -64/-56 格"拦截会拒绝几乎所有真实模板。
// ⇒ 该风险以**运行时边界声明**带账（`pager.rs` 模块文档"未实现"清单）：
// 行放不下且页上有值为 String 的普通格时，产品会测量回填/拆分，本实现直接硬切。
// 精确守卫需要把模板 cell_type 接进计算网格并在断页点复查——记为后续带账。
