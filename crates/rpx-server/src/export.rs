//! 导出面（M6-03）：`POST /export?fmt=csv|xlsx` 的纯函数与响应组装。
//!
//! # 分工（spec-m6 §2.1 / §4）
//!
//! * **计算主链路**（请求 → 计算态网格）与 `/render` 共享
//!   [`crate::http::calculate_grid`]（同一次调用，出口分家不产生口径分叉）；
//! * 本模块只做两件事：**网格 → 文件字节**（转发 `rpx-export`）与
//!   **HTTP 响应头**（`Content-Type` / `Content-Disposition`）。
//! * 文件字节的格式契约在 `rpx-export`（CSV = spec-m6 §2.3；xlsx = §2.4）。
//!
//! # 文件名口径（`Content-Disposition`）
//!
//! 双形态（老客户端读 `filename`、新客户端读 `filename*`）：
//!
//! * ASCII fallback：白名单 `[A-Za-z0-9._-]` 之外一律 `_`，全空 → `export`
//!   —— 保证任何报名字节都不会破坏 header 语法；
//! * RFC 5987 `filename*=UTF-8''<pct>`：对**原始名字**做字节级百分号编码
//!   （仅 `attr-char` 集保留）—— 控制字符 / 引号 / 非 ASCII 全被编码，
//!   天然无 header 注入面（不需要单独的 sanitize 表）。
//!
//! ⚠ 已知边界：报名字符串两个形态都在，但都不做 Windows 保留名（CON/NUL…）
//! 检查 —— 文件名最终由客户端（浏览器/下载器）落盘，服务端只保证**无注入**。

use std::collections::HashMap;

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::datasource::DataSourceRegistry;
use crate::form::RenderRequest;
use crate::http::RenderError;

/// 导出格式（query `fmt`；缺省 [`ExportFormat::Xlsx`]——蓝图 §8.1 的示例即 xlsx）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// 自定 RFC 4180 子集（spec-m6 §2.3）。
    Csv,
    /// rust_xlsxwriter 单 sheet（spec-m6 §2.4）。
    Xlsx,
}

impl ExportFormat {
    /// `fmt` 值解析：**严格小写**（`csv` / `xlsx`）；其余 None，调用方报 400
    /// （不静默回落——「诚实失败优于静默错」）。
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "csv" => Some(Self::Csv),
            "xlsx" => Some(Self::Xlsx),
            _ => None,
        }
    }

    /// 响应 `Content-Type`。
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Csv => "text/csv; charset=utf-8",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        }
    }

    /// 文件名扩展名（不含点）。
    pub fn ext(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
        }
    }
}

/// 导出主链路（**纯函数**：集成测试不经 socket 直调，与 socket 响应体对拍；
/// 圆括号口径同 [`crate::http::render_request`]）。无数据源注册表 ⇒ 模板含
/// SQL 数据集时显式报 [`RenderError::NoDataSource`]。
pub fn export_request(req: &RenderRequest, fmt: ExportFormat) -> Result<Vec<u8>, RenderError> {
    let rd = crate::reportdef::open_report(&req.report).map_err(RenderError::Source)?;
    export_core(req, &rd, &HashMap::new(), fmt)
}

/// 异步版（HTTP handler 入口）：SQL 数据集预取后进同步 [`export_core`]
/// （async 取数只在 server 编排层发生——与 `/render` 同构）。
pub async fn export_request_ds(
    req: &RenderRequest,
    dsr: Option<&DataSourceRegistry>,
    fmt: ExportFormat,
) -> Result<Vec<u8>, RenderError> {
    let rd = crate::reportdef::open_report(&req.report).map_err(RenderError::Source)?;
    let prefetch = crate::http::prefetch_sql(&rd, dsr).await?;
    export_core(req, &rd, &prefetch, fmt)
}

/// 同步主链路：算网格（与 `/render` 共享段）→ 格式化字节。
fn export_core(
    req: &RenderRequest,
    rd: &rpx_core::ReportDefine,
    prefetch: &HashMap<(String, String), rpx_core::rowset::RowSet>,
    fmt: ExportFormat,
) -> Result<Vec<u8>, RenderError> {
    let grid = crate::http::calculate_grid(req, rd, prefetch)?;
    Ok(match fmt {
        ExportFormat::Csv => rpx_export::to_csv(&grid).into_bytes(),
        ExportFormat::Xlsx => rpx_export::to_xlsx(&grid),
    })
}

/// 导出响应组装：`Content-Type` + `Content-Disposition`（双形态文件名）。
pub fn export_response(bytes: Vec<u8>, report_name: &str, fmt: ExportFormat) -> Response {
    let ext = fmt.ext();
    let disposition = format!(
        "attachment; filename=\"{}.{ext}\"; filename*=UTF-8''{}.{ext}",
        ascii_filename(report_name),
        rfc5987_encode(report_name)
    );
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, fmt.content_type()),
            (header::CONTENT_DISPOSITION, disposition.as_str()),
        ],
        bytes,
    )
        .into_response()
}

/// ASCII fallback 文件名：白名单 `[A-Za-z0-9._-]`，其余 `_`；结果全为 `_`/`.`
/// （或空）→ `export`。
fn ascii_filename(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if mapped.is_empty() || mapped.chars().all(|c| c == '_' || c == '.') {
        "export".to_string()
    } else {
        mapped
    }
}

/// RFC 5987 的 `attr-char` 集内保留、其余百分号编码（UTF-8 字节）。
///
/// attr-char（RFC 5987 §3.2.1）= `ALPHA / DIGIT / "!" / "#" / "$" / "&" /
/// "+" / "-" / "." / "^" / "_" / "`" / "|" / "~"`。
fn rfc5987_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'!'
            | b'#'
            | b'$'
            | b'&'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests;
