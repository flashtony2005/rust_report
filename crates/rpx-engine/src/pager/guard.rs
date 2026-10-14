//! 分页配置的**支持性守卫**（`ensure_supported`）——2026-10-14 拆自 `pager.rs`
//! （行数门禁 G4；调用点在 `calc::calculate()` 入口）。
//!
//! [`super::paginate`] 按 `SimplePager._$5` 的 `pagerStyle == 2 && columns == 1`
//! 分支移植（探针口径，见 [`super::PAPER_W_PT`]）。以下三个 `PrintSetup` 字段会让
//! 产品的分页**改走别的分支**，而本实现不读它们 ⇒ 若静默按 `2` 跑，会产出**错误
//! 的分页**。故显式报 [`EngineError::UnsupportedPaging`]（`spec-m2.md` §6.2 带账项）。
//!
//! ⛔ 这是"未实现"，**不是产品行为** —— 产品支持这些配置；它们带账到有产品探针 +
//! 夹具时实现（或 M3 的真实口径接线时处理，见 `spec-m3.md` §3.2）。

use crate::error::{EngineError, Result};
use rpx_core::print_setup::PrintSetup;

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
    Ok(())
}
