//! HTTP 面（M3-05）：axum 路由 + `POST /render` 主链路。
//!
//! 主链路十步（`docs/spec-m3.md` §3.2）到本文件的映射：
//!
//! | 步骤        | 实现                                                                  |
//! | ----------- | --------------------------------------------------------------------- |
//! | ① 读字节    | multipart 上传（[`crate::form`]）—— 资产库（libSQL BLOB）是 M3-04，同槽替换 |
//! | ② 解模板    | `rpx_core::open_bytes`                                                |
//! | ③④ 物化     | [`rpx_engine::dataset::load_datasets_with`]（闭包查上传 map）            |
//! | ⑤⑥⑦⑧ 计算  | `rpx_engine::calculate_with`（M2 已闭合，含 pager 守卫）                 |
//! | ⑨ 分页      | **M3-06**：届时只改 [`render_request`] 一处调用                          |
//! | ⑩ 快照      | [`crate::snapshot::build`]（契约 `docs/spec-m4.md` §4）                  |
//!
//! 错误口径：客户端输入问题（模板坏 / 缺文件 / 参数未登记 / 编码坏）→ **400**，
//! 消息显式（前端原样上屏）；快照不变式违反 → **500**（服务端缺陷，不该发生）。

use std::net::SocketAddr;

use axum::{
    extract::Multipart,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::json;

use rpx_core::RpxError;
use rpx_engine::EngineError;

use crate::{form, snapshot};

/// 应用（[`serve`] 与集成测试共用同一份路由；监听地址由调用方决定）。
pub fn app() -> Router {
    Router::new()
        .route("/render", post(render))
        .route("/health", get(health))
}

/// 起服务（`rpx-server serve` 子命令）：阻塞到进程结束。
pub async fn serve(addr: SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app()).await
}

async fn health() -> &'static str {
    "ok"
}

/// `POST /render`：multipart → 快照 JSON（`IWorkbookData`）。
async fn render(mut multipart: Multipart) -> Response {
    let req = match form::parse(&mut multipart).await {
        Ok(r) => r,
        Err(e) => return error_json(StatusCode::BAD_REQUEST, &format!("表单错误：{e}")),
    };
    match render_request(&req) {
        Ok(json_text) => (
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json_text,
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

/// 主链路本体（**纯函数**：集成测试不经 socket 直调，与 socket 响应体对拍，
/// 见 `tests/render_http.rs`）。
///
/// 上传完整性预检：模板引用的 CSV 必须都在表单里。引擎侧的
/// [`EngineError::MissingDataset`] 只带数据集名（`ds1`），而上传场景用户只认
/// **文件名** —— 所以在引擎之前先按文件名报缺（消息里两者都给）。
pub fn render_request(req: &form::RenderRequest) -> Result<String, RenderError> {
    let rd = rpx_core::open_bytes(&req.report).map_err(RenderError::Core)?;
    for entry in &rd.datasets {
        let Some(file) = entry.file.as_ref() else {
            continue;
        };
        let Some(file_name) = file.file_name.as_deref() else {
            continue;
        };
        if !req.datasets.contains_key(file_name) {
            return Err(RenderError::MissingFile {
                file: file_name.to_string(),
                dataset: file.name.clone().unwrap_or_else(|| file_name.to_string()),
            });
        }
    }
    let ds = rpx_engine::dataset::load_datasets_with(&rd, |name| req.datasets.get(name).cloned())
        .map_err(RenderError::Engine)?;
    let grid = rpx_engine::calculate_with(&rd, &ds, &req.params).map_err(RenderError::Engine)?;
    snapshot::build(&rd, &grid, &req.report_name).map_err(RenderError::Snapshot)
}

/// 渲染错误 → HTTP 映射（400 = 客户端输入；500 = 服务端缺陷）。
#[derive(Debug)]
pub enum RenderError {
    /// 模板字节解不开（魔数 / 截断 / 页长不符）。
    Core(RpxError),
    /// 引擎拒绝：缺数据集 / 未登记参数 / 编码坏 / 循环引用 / 不支持的分页…。
    Engine(EngineError),
    /// 上传不完整：模板引用的 CSV 不在表单里（按**文件名**报，见上文）。
    MissingFile { file: String, dataset: String },
    /// 快照映射不变式被违反（`geom::merge_data` 的网格校验）——不该发生。
    Snapshot(String),
}

impl IntoResponse for RenderError {
    fn into_response(self) -> Response {
        match self {
            RenderError::Core(e) => {
                error_json(StatusCode::BAD_REQUEST, &format!("模板解析失败：{e}"))
            }
            RenderError::Engine(e) => {
                error_json(StatusCode::BAD_REQUEST, &format!("计算失败：{e}"))
            }
            RenderError::MissingFile { file, dataset } => error_json(
                StatusCode::BAD_REQUEST,
                &format!("缺少数据集文件：{file}（模板数据集 {dataset} 引用）"),
            ),
            RenderError::Snapshot(s) => error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("快照映射失败：{s}"),
            ),
        }
    }
}

/// 错误响应体：`{"error": "..."}`（前端原样上屏，见 `web/src/App.tsx`）。
fn error_json(status: StatusCode, message: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({ "error": message }).to_string(),
    )
        .into_response()
}
