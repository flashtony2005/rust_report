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

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::{
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::json;

use rpx_core::RpxError;
use rpx_ds::{AssetError, AssetStore};
use rpx_engine::EngineError;

use crate::{datasource::DataSourceRegistry, form, snapshot};

/// 应用状态：资产库 + 可选数据源注册表（SQL 数据集的连接由调用方注入，
/// 产品同构——模板只存 dsn 引用，见 `datasource.rs` 模块文档）。
#[derive(Clone)]
pub struct AppState {
    pub store: std::sync::Arc<AssetStore>,
    pub dsr: Option<std::sync::Arc<DataSourceRegistry>>,
}

/// 应用（[`serve`] 与集成测试共用同一份路由；监听地址由调用方决定）。
///
/// 资产库（M3-04）：`reports` 三路由挂在同一个 [`AssetStore`] 上——
/// 路径不含 `/api` 前缀（web 的 vite 代理剥 `/api`，与 `/render` 同约定）。
pub fn app(
    store: std::sync::Arc<AssetStore>,
    dsr: Option<std::sync::Arc<DataSourceRegistry>>,
) -> Router {
    Router::new()
        .route("/render", post(render))
        .route("/reports", post(upload_report).get(list_reports))
        .route("/reports/{id}/meta", get(report_meta))
        .route("/health", get(health))
        .with_state(AppState { store, dsr })
}

/// 起服务（`rpx-server serve` 子命令）：阻塞到进程结束。
pub async fn serve(
    addr: SocketAddr,
    store: std::sync::Arc<AssetStore>,
    dsr: Option<std::sync::Arc<DataSourceRegistry>>,
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app(store, dsr)).await
}

async fn health() -> &'static str {
    "ok"
}

/// `POST /reports`（M3-04）：multipart 上传 `.rpx` → 魔数校验 → 存 BLOB +
/// 参数清单（蓝图 §八 路由表）。
///
/// 表单字段：`file`（**文件名**去扩展名 = 报表名，与 `/render` 的 `report_name`
/// 同口径）。返回 `{"id": …, "name": …}`。
async fn upload_report(State(state): State<AppState>, mut multipart: Multipart) -> Response {
    let store = state.store;
    let (name, bytes) = match read_upload(&mut multipart).await {
        Ok(v) => v,
        // Err 是 `Box<Response>`（clippy result_large_err：Response 体量大）。
        Err(e) => return (*e).into_response(),
    };
    match store.put(&name, &bytes).await {
        Ok(id) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "id": id, "name": name }).to_string(),
        )
            .into_response(),
        Err(e) => asset_error_response(e),
    }
}

/// `GET /reports`：列表（名称/版本/时间）。
async fn list_reports(State(state): State<AppState>) -> Response {
    let store = state.store;
    match store.list().await {
        Ok(items) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!(items
                .iter()
                .map(|a| json!({
                    "id": a.id, "name": a.name, "version": a.version,
                    "created_at": a.created_at,
                }))
                .collect::<Vec<_>>())
            .to_string(),
        )
            .into_response(),
        Err(e) => asset_error_response(e),
    }
}

/// `GET /reports/{id}/meta`：参数清单（**读库即答**——清单在上传时解析落库，
/// 本路由不碰解析器；蓝图 §八：静态/动态两套 MetaData 供前端生成参数表单）。
async fn report_meta(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let store = state.store;
    match store.meta(id).await {
        Ok(items) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!(items
                .iter()
                .map(|p| json!({
                    "name": p.name, "param_type": p.param_type,
                    "is_static": p.is_static, "data_type": p.data_type,
                    "default_val": p.default_val, "description": p.description,
                }))
                .collect::<Vec<_>>())
            .to_string(),
        )
            .into_response(),
        Err(e) => asset_error_response(e),
    }
}

/// `AssetError` → HTTP 映射（本地辅助函数——`AssetError` 是外来类型，
/// 本仓不能给它 impl `IntoResponse`，E0117）：
/// 坏模板 400 / 不存在 404 / 库故障 500。
fn asset_error_response(e: AssetError) -> Response {
    match e {
        AssetError::Parse(e) => error_json(StatusCode::BAD_REQUEST, &format!("模板解析失败：{e}")),
        AssetError::NotFound(id) => {
            error_json(StatusCode::NOT_FOUND, &format!("报表不存在：id={id}"))
        }
        AssetError::Sql(e) => error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("资产库操作失败：{e}"),
        ),
    }
}

/// 从 multipart 取上传：字段名 `file`（没有则退回第一个带文件名的字段）。
/// 报表名 = 文件名去扩展名（[`crate::form`] 的 `report_name` 同口径）。
async fn read_upload(multipart: &mut Multipart) -> Result<(String, Vec<u8>), Box<Response>> {
    let mut fallback: Option<(String, Vec<u8>)> = None;
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        Box::new(error_json(
            StatusCode::BAD_REQUEST,
            &format!("表单错误：{e}"),
        ))
    })? {
        let Some(file_name) = field.file_name().map(str::to_string) else {
            continue;
        };
        let is_file_field = field.name() == Some("file");
        let bytes = field.bytes().await.map_err(|e| {
            Box::new(error_json(
                StatusCode::BAD_REQUEST,
                &format!("表单错误：{e}"),
            ))
        })?;
        let name = std::path::Path::new(&file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("report")
            .to_string();
        if is_file_field {
            return Ok((name, bytes.to_vec()));
        }
        if fallback.is_none() {
            fallback = Some((name, bytes.to_vec()));
        }
    }
    fallback.ok_or_else(|| {
        Box::new(error_json(
            StatusCode::BAD_REQUEST,
            "缺少上传字段：`file`（.rpx 字节）",
        ))
    })
}

/// `POST /render`：multipart → 快照 JSON（`IWorkbookData`）。
async fn render(State(state): State<AppState>, mut multipart: Multipart) -> Response {
    let req = match form::parse(&mut multipart).await {
        Ok(r) => r,
        Err(e) => return error_json(StatusCode::BAD_REQUEST, &format!("表单错误：{e}")),
    };
    match render_request_ds(&req, state.dsr.as_deref()).await {
        Ok(json_text) => (
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json_text,
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

/// 主链路本体（**纯函数**：集成测试不经 socket 直调，与 socket 响应体对拍，
/// 见 `tests/render_http.rs`）。不提供数据源注册表 ⇒ 模板含 SQL 数据集时
/// 显式报 [`RenderError::NoDataSource`]（SQL 数据集走 [`render_request_ds`]）。
pub fn render_request(req: &form::RenderRequest) -> Result<String, RenderError> {
    let rd = rpx_core::open_bytes(&req.report).map_err(RenderError::Core)?;
    render_core(req, &rd, &HashMap::new())
}

/// 主链路异步版（HTTP handler 入口）：先**预取**全部 SQL 数据集（注册表
/// 逐条物化 [`RowSet`]），再进同步 [`render_core`]——引擎侧的 `sql_fetch`
/// 闭包是同步契约（G8），async 取数只在 server 编排层发生（产品同构：
/// 连接管理在编排层，引擎只见 `getConnectionFactory`）。
pub async fn render_request_ds(
    req: &form::RenderRequest,
    dsr: Option<&DataSourceRegistry>,
) -> Result<String, RenderError> {
    let rd = rpx_core::open_bytes(&req.report).map_err(RenderError::Core)?;
    let prefetch = prefetch_sql(&rd, dsr).await?;
    render_core(req, &rd, &prefetch)
}

/// SQL 数据集预取：`(dsn, sql_text) → RowSet`。注册表缺失 / dsn 未登记 /
/// 取数失败都**显式报错**（数据集缺失会被表达式求值期报错掩盖成更难定位
/// 的形态，见 `rpx_engine::dataset::load_datasets_full` 的同款口径）。
async fn prefetch_sql(
    rd: &rpx_core::ReportDefine,
    dsr: Option<&DataSourceRegistry>,
) -> Result<HashMap<(String, String), rpx_core::rowset::RowSet>, RenderError> {
    let mut map = HashMap::new();
    if !rd.datasets.iter().any(|e| e.sql.is_some()) {
        return Ok(map);
    }
    let Some(dsr) = dsr else {
        let dsn = rd
            .datasets
            .iter()
            .find_map(|e| e.sql.as_ref().and_then(|s| s.data_source_name.clone()))
            .unwrap_or_default();
        return Err(RenderError::NoDataSource { dsn });
    };
    for e in &rd.datasets {
        let Some(sql) = e.sql.as_ref() else {
            continue;
        };
        let dsn = sql.data_source_name.as_deref().unwrap_or("").to_string();
        let text = sql.sql.as_deref().unwrap_or("").to_string();
        if map.contains_key(&(dsn.clone(), text.clone())) {
            continue; // 同 (dsn,sql) 只取一次（同连接同 SQL 的模板重复声明）
        }
        let src = dsr
            .get(&dsn)
            .ok_or_else(|| RenderError::UnknownDataSource {
                dsn: dsn.clone(),
                available: dsr.names().iter().map(|s| s.to_string()).collect(),
            })?;
        let rs = src.fetch_sql(&text).await.map_err(RenderError::SqlFetch)?;
        map.insert((dsn, text), rs);
    }
    Ok(map)
}

/// 同步主链路（输入 = 表单 + 已预取的 SQL 数据集）。
fn render_core(
    req: &form::RenderRequest,
    rd: &rpx_core::ReportDefine,
    prefetch: &HashMap<(String, String), rpx_core::rowset::RowSet>,
) -> Result<String, RenderError> {
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
    let ds = rpx_engine::dataset::load_datasets_full(
        rd,
        &|name| req.datasets.get(name).cloned(),
        Some(&|dsn, sql| {
            prefetch
                .get(&(dsn.to_string(), sql.to_string()))
                .cloned()
                .ok_or_else(|| EngineError::SqlDatasetNoConnection {
                    name: "<sql-dataset>".to_string(),
                    dsn: dsn.to_string(),
                })
        }),
    )
    .map_err(RenderError::Engine)?;
    let mut grid = rpx_engine::calculate_with(rd, &ds, &req.params).map_err(RenderError::Engine)?;
    // M3-06：`/render` 的 pagination = **模板真值口径**（纸码表 + 模板边距，
    // `PagerInfo(PrintSetup)` 路径），覆盖 `calculate_with` 内的探针口径
    // （595×842 是产品探针覆盖出来的夹具对拍基准——双口径见 `pager.rs` 模块文档）。
    // 守卫（pagerStyle/多栏/列区带 + 可用尺寸非正）在此再复核一遍，报 400。
    let pg = rpx_engine::pager::paginate_with_setup(&grid, &rd.print_setup)
        .map_err(RenderError::Engine)?;
    grid.page_count = pg.page_count();
    grid.pagination = Some(pg);
    snapshot::build(rd, &grid, &req.report_name).map_err(RenderError::Snapshot)
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
    /// 模板含 SQL 数据集但服务端没有数据源注册表（`serve` 未给
    /// `--datasources`）——显式 400，消息里带 dsn 与补救路径。
    NoDataSource { dsn: String },
    /// 注册表存在但模板引用的 dsn 未登记（消息列出已登记数据源名）。
    UnknownDataSource { dsn: String, available: Vec<String> },
    /// SQL 取数失败（连接/查询/列型）——sqlx 错误文本原样透传给前端上屏。
    SqlFetch(rpx_ds::DsError),
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
            RenderError::NoDataSource { dsn } => error_json(
                StatusCode::BAD_REQUEST,
                &format!(
                    "模板含 SQL 数据集（数据源 {dsn}），但服务端未配置数据源注册表——\
                     启动时加 --datasources <path.json>"
                ),
            ),
            RenderError::UnknownDataSource { dsn, available } => error_json(
                StatusCode::BAD_REQUEST,
                &format!(
                    "数据源未登记：{dsn}（已登记：{}）",
                    if available.is_empty() {
                        "无".to_string()
                    } else {
                        available.join(", ")
                    }
                ),
            ),
            RenderError::SqlFetch(e) => {
                error_json(StatusCode::BAD_REQUEST, &format!("SQL 取数失败：{e}"))
            }
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
