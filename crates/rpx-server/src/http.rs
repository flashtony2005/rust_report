//! HTTP 面（M3-05）：axum 路由 + `POST /render` + `POST /export`（M6-03）主链路。
//!
//! 主链路十步（`docs/spec-m3.md` §3.2）到本文件的映射：
//!
//! | 步骤        | 实现                                                                  |
//! | ----------- | --------------------------------------------------------------------- |
//! | ① 读字节    | multipart 上传（[`crate::form`]）—— 资产库（libSQL BLOB）是 M3-04，同槽替换 |
//! | ② 解模板    | `rpx_core::open_bytes`                                                |
//! | ③④ 物化     | [`rpx_engine::dataset::load_datasets_with`]（闭包查上传 map）            |
//! | ⑤⑥⑦⑧ 计算  | `rpx_engine::calculate_with`（M2 已闭合，含 pager 守卫）                 |
//! | ⑨ 分页      | **M3-06**：`paginate_with_setup`（②-⑨ 收拢在 [`calculate_grid`]）        |
//! | ⑩ 出口      | `/render` → [`crate::snapshot::build`]；`/export` → [`crate::export`]    |
//!
//! `POST /export?fmt=csv|xlsx`（M6-03）：与 `/render` **共享 ①-⑨**（同一次
//! [`calculate_grid`] 调用），只在 ⑩ 分家——快照 JSON vs 文件字节。
//!
//! 错误口径：客户端输入问题（模板坏 / 缺文件 / 参数未登记 / 编码坏 / fmt 非法）
//! → **400**，消息显式（前端原样上屏）；快照不变式违反 → **500**（服务端缺陷）。

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::json;

use rpx_ds::{AssetError, AssetStore};
use rpx_engine::EngineError;

use crate::reportdef::{self, SourceError};
use crate::{datasource::DataSourceRegistry, export, form, snapshot};

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
        .route("/export", post(export))
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
    // 资产库的唯一事实源是 `.rpx`（`reports.rpx BLOB`）：JSON 定义是**入口格式**，
    // 在此归一化落库，下游（meta 解析 / render / export）因此只有一条读侧路径。
    let bytes = match normalize_to_rpx(&bytes) {
        Ok(b) => b,
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

/// 上传物 → **规范 `.rpx` 字节**（`POST /reports` 的入库前归一化）。
///
/// * 已是 `.rpx`（`RQQR` 魔数）→ **原样入库**，不重写（零风险，保持既有行为）；
/// * 定义 JSON → [`reportdef::open_report`] 解析 → `rpx_core::to_bytes` 序列化。
///   「解析后再序列化与原字节逐字节一致」已由 `crates/rpx-core/tests/roundtrip.rs`
///   对 37 个真实夹具钉死，故这里对 `.rpx` 不做往返改写是保守但无损的选择。
/// * 两者都不是 → 显式 400（与 `/render` 同一套错误文本）。
fn normalize_to_rpx(bytes: &[u8]) -> Result<Vec<u8>, Box<Response>> {
    if bytes.starts_with(b"RQQR") {
        return Ok(bytes.to_vec());
    }
    let rd = reportdef::open_report(bytes)
        .map_err(|e| Box::new(RenderError::Source(e).into_response()))?;
    rpx_core::to_bytes(&rd)
        .map_err(|e| Box::new(RenderError::Serialize(e.to_string()).into_response()))
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

/// `POST /export?fmt=csv|xlsx`（M6-03）：multipart → **文件字节** +
/// `Content-Disposition`（文件名双形态，见 [`crate::export`]）。
///
/// `fmt` 缺省 = `xlsx`（蓝图 §8.1 示例口径）；非法值显式 400（不静默回落）。
async fn export(
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
    mut multipart: Multipart,
) -> Response {
    let fmt = match q.get("fmt").map(String::as_str) {
        None => export::ExportFormat::Xlsx,
        Some(s) => match export::ExportFormat::parse(s) {
            Some(f) => f,
            None => {
                return error_json(
                    StatusCode::BAD_REQUEST,
                    &format!("不支持的导出格式：{s}（可选 csv / xlsx）"),
                );
            }
        },
    };
    let req = match form::parse(&mut multipart).await {
        Ok(r) => r,
        Err(e) => return error_json(StatusCode::BAD_REQUEST, &format!("表单错误：{e}")),
    };
    match export::export_request_ds(&req, state.dsr.as_deref(), fmt).await {
        Ok(bytes) => export::export_response(bytes, &req.report_name, fmt),
        Err(e) => e.into_response(),
    }
}

/// 主链路本体（**纯函数**：集成测试不经 socket 直调，与 socket 响应体对拍，
/// 见 `tests/render_http.rs`）。不提供数据源注册表 ⇒ 模板含 SQL 数据集时
/// 显式报 [`RenderError::NoDataSource`]（SQL 数据集走 [`render_request_ds`]）。
pub fn render_request(req: &form::RenderRequest) -> Result<String, RenderError> {
    let rd = reportdef::open_report(&req.report).map_err(RenderError::Source)?;
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
    let rd = reportdef::open_report(&req.report).map_err(RenderError::Source)?;
    let prefetch = prefetch_sql(&rd, dsr).await?;
    render_core(req, &rd, &prefetch)
}

/// SQL 数据集预取：`(dsn, sql_text) → RowSet`。注册表缺失 / dsn 未登记 /
/// 取数失败都**显式报错**（数据集缺失会被表达式求值期报错掩盖成更难定位
/// 的形态，见 `rpx_engine::dataset::load_datasets_full` 的同款口径）。
pub(crate) async fn prefetch_sql(
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

/// 同步主链路（输入 = 表单 + 已预取的 SQL 数据集）→ 快照 JSON。
fn render_core(
    req: &form::RenderRequest,
    rd: &rpx_core::ReportDefine,
    prefetch: &HashMap<(String, String), rpx_core::rowset::RowSet>,
) -> Result<String, RenderError> {
    let grid = calculate_grid(req, rd, prefetch)?;
    snapshot::build(rd, &grid, &req.report_name).map_err(RenderError::Snapshot)
}

/// 计算主链路（`/render` 与 `/export` 的**共享段**，主链路 ②-⑨ 步）：
/// 请求 → 计算态网格（含分页结果与 `page_count` 覆盖）。
///
/// M3-06 口径（原 `render_core` 内注释原样保留）：`/render` 的 pagination =
/// **模板真值口径**（纸码表 + 模板边距，`PagerInfo(PrintSetup)` 路径），覆盖
/// `calculate_with` 内的探针口径（595×842 是产品探针覆盖出来的夹具对拍基准
/// ——双口径见 `pager.rs` 模块文档）。守卫（pagerStyle/多栏/列区带 + 可用
/// 尺寸非正）在此复核，报 400。
///
/// `/export`（M6-03）复用同一趟：导出打的是**整个计算矩阵**（不分页、不按纸切，
/// spec-m6 §4），但走同一守卫与同一页数覆盖 ⇒ 出口分家不产生口径分叉。
pub(crate) fn calculate_grid(
    req: &form::RenderRequest,
    rd: &rpx_core::ReportDefine,
    prefetch: &HashMap<(String, String), rpx_core::rowset::RowSet>,
) -> Result<rpx_engine::arena::CalcGrid, RenderError> {
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
    let pg = rpx_engine::pager::paginate_with_setup(&grid, &rd.print_setup)
        .map_err(RenderError::Engine)?;
    grid.page_count = pg.page_count();
    grid.pagination = Some(pg);
    Ok(grid)
}

/// 渲染错误 → HTTP 映射（400 = 客户端输入；500 = 服务端缺陷）。
#[derive(Debug)]
pub enum RenderError {
    /// 报表来源解不开：`.rpx` 字节坏 / 定义 JSON 不合法 / 格式无法判定。
    Source(SourceError),
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
    /// 定义 JSON → `.rpx` 序列化失败（`POST /reports` 的归一化步骤）。
    Serialize(String),
    /// 快照映射不变式被违反（`geom::merge_data` 的网格校验）——不该发生。
    Snapshot(String),
}

impl IntoResponse for RenderError {
    fn into_response(self) -> Response {
        match self {
            RenderError::Source(e) => match e {
                // 两条入口的错误文本分开口径：`.rpx` 坏是「模板解析失败」，
                // JSON 坏是「报表定义不合法」（带 JSON 路径），便于前端定位。
                SourceError::Core(e) => {
                    error_json(StatusCode::BAD_REQUEST, &format!("模板解析失败：{e}"))
                }
                SourceError::Def(e) => {
                    error_json(StatusCode::BAD_REQUEST, &format!("报表定义不合法：{e}"))
                }
                SourceError::UnknownFormat => error_json(StatusCode::BAD_REQUEST, &e.to_string()),
            },
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
            RenderError::Serialize(detail) => error_json(
                StatusCode::BAD_REQUEST,
                &format!("模板序列化失败：{detail}"),
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
