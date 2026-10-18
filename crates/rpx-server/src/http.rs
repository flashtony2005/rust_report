//! Library JSON source editing and fixed run artifacts. No calculation handlers.
use axum::{
    extract::{DefaultBodyLimit, Extension, Path, Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rpx_runtime::{
    error::{Error, Result},
    files, library,
};
use serde_json::{json, Value as J};
use std::{collections::HashMap, path::PathBuf};

/// 请求体上限（含未匹配路由的排空）。
const BODY_LIMIT: usize = 64 * 1024 * 1024;

/// 本地开发默认无鉴权；生产用 [`app_with_auth`]。
pub fn app(root: PathBuf) -> Router {
    app_with_auth(root, None)
}

/// Bearer token 鉴权。`/health` 与 `/auth/status` 始终公开。
pub fn app_with_auth(root: PathBuf, token: Option<String>) -> Router {
    let required = token.is_some();
    let mut router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth/status", get(auth_status))
        .route("/reports", get(list))
        .route("/reports/{id}", get(show))
        .route("/reports/{id}/template", axum::routing::put(save))
        .route("/reports/{id}/runs", get(runs))
        .route("/reports/{id}/runs/{run}", get(record))
        .route("/reports/{id}/runs/{run}/source/{*path}", get(source))
        .route("/reports/{id}/runs/{run}/artifacts/{name}", get(artifact))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .with_state(root)
        .layer(Extension(AuthRequired(required)));
    if let Some(token) = token {
        router = router.layer(middleware::from_fn_with_state(
            AuthState(token),
            require_auth,
        ));
    }
    router
}

#[derive(Clone)]
struct AuthState(String);
#[derive(Clone, Copy)]
struct AuthRequired(bool);

async fn auth_status(Extension(AuthRequired(required)): Extension<AuthRequired>) -> Response {
    Json(json!({"required": required})).into_response()
}

/// 未匹配路由：**先把请求体读完**，再回 404。
///
/// 不读 body 直接返回，会让 hyper 在一个接收队列还有未读字节的 socket 上关闭连接，
/// 内核发的是 RST 而不是 FIN ⇒ 客户端在读到响应之前就吃 `ECONNRESET`，
/// **连 404 本身都丢掉**（RFC 9112 §9.6：连接在响应送达前被重置）。
/// 判据见 `tests/library_http.rs`：`POST /render`、`POST /export` 这两个路由
/// **故意不存在**（回归护栏），带 8 MiB 请求体也必须稳定读到 404。
///
/// 遗留边界：body 超过 [`BODY_LIMIT`] 时排空会提前放弃，此时仍可能被 RST 吃掉；
/// 根治要 `Expect: 100-continue` 或按 `Content-Length` 预检。
async fn not_found(request: Request) -> Response {
    let _ = axum::body::to_bytes(request.into_body(), BODY_LIMIT).await;
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error":{"code":"NOT_FOUND","message":"没有这个接口"}})),
    )
        .into_response()
}

async fn require_auth(State(auth): State<AuthState>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if path == "/health" || path == "/auth/status" {
        return next.run(request).await;
    }
    let ok = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|token| token == auth.0);
    if ok {
        return next.run(request).await;
    }
    // 401 也是早退分支：同样要先把 body 读完，否则响应会被 RST 吃掉。
    let _ = axum::body::to_bytes(request.into_body(), BODY_LIMIT).await;
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error":{"code":"AUTH_REQUIRED","message":"需要 Authorization: Bearer <token>"}})),
    )
        .into_response()
}
fn response(value: Result<J>) -> Response {
    match value {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            let status = match e.exit {
                7 => StatusCode::PRECONDITION_FAILED,
                6 => StatusCode::NOT_FOUND,
                1 => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(json!({"error":e.diagnostic}))).into_response()
        }
    }
}
async fn blocking(f: impl FnOnce() -> Result<J> + Send + 'static) -> Response {
    response(
        tokio::task::spawn_blocking(f)
            .await
            .unwrap_or_else(|e| Err(Error::new(1, "INTERNAL", "http", e))),
    )
}
fn page(q: &HashMap<String, String>) -> Result<(usize, usize)> {
    let number = |k, default| {
        q.get(k)
            .map(|v| {
                v.parse::<usize>()
                    .map_err(|_| Error::new(2, "USAGE", "query", "分页参数需要正整数"))
            })
            .unwrap_or(Ok(default))
    };
    Ok((number("limit", 50)?, number("offset", 0)?))
}
async fn list(State(root): State<PathBuf>, Query(q): Query<HashMap<String, String>>) -> Response {
    blocking(move || {
        let (limit, offset) = page(&q)?;
        library::list(
            &root,
            q.get("query").map(String::as_str),
            q.get("tag").map(String::as_str),
            limit,
            offset,
        )
    })
    .await
}
async fn show(State(root): State<PathBuf>, Path(id): Path<String>) -> Response {
    blocking(move || library::show(&root, &id)).await
}
async fn save(State(root): State<PathBuf>, Path(id): Path<String>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    // 先把 body 读完再判任何东西：415/400 都是早退分支，而在接收队列非空的
    // socket 上关闭连接会发 RST 而不是 FIN，客户端连响应都读不到（RFC 9112 §9.6）。
    // 判据：tests/library_http.rs 用 text/plain 发 PUT，必须稳定读到 415。
    let bytes = match axum::body::to_bytes(body, BODY_LIMIT).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({"error":{"code":"PAYLOAD_TOO_LARGE","message":format!("请求体超过 {BODY_LIMIT} 字节上限")}})),
            )
                .into_response()
        }
    };
    if !is_json(&parts.headers) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(
                json!({"error":{"code":"UNSUPPORTED_MEDIA_TYPE","message":"Content-Type 必须是 application/json"}}),
            ),
        )
            .into_response();
    }
    let value: J = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":{"code":"JSON_INVALID","message":e.to_string()}})),
            )
                .into_response()
        }
    };
    let expected = parts
        .headers
        .get(header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_matches('"').to_string());
    let Some(expected) = expected else {
        return (
            StatusCode::PRECONDITION_REQUIRED,
            Json(
                json!({"error":{"code":"REVISION_REQUIRED","message":"保存必须带 If-Match 修订"}}),
            ),
        )
            .into_response();
    };
    blocking(move || library::save(&root, &id, &expected, &value, false)).await
}

/// axum `Json` 认可的媒体类型：`application/json` 或 `application/*+json`，
/// `charset` 只接受 utf-8（与 axum 0.8 的判定保持一致，别在这里放宽）。
fn is_json(headers: &HeaderMap) -> bool {
    let Some(content_type) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let mut parts = content_type.split(';');
    let mime = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let json = mime == "application/json"
        || mime
            .strip_prefix("application/")
            .is_some_and(|sub| sub.ends_with("+json"));
    json && parts.all(|p| {
        let p = p.trim().to_ascii_lowercase();
        !p.starts_with("charset=") || p == "charset=utf-8"
    })
}

async fn runs(
    State(root): State<PathBuf>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    blocking(move || {
        let (limit, offset) = page(&q)?;
        library::show(&root, &id)?;
        files::page(&library::runs(&root, &id)?, limit, offset)
    })
    .await
}
async fn record(State(root): State<PathBuf>, Path((id, run)): Path<(String, String)>) -> Response {
    blocking(move || library::run_record(&root, &id, &run)).await
}
async fn artifact(
    State(root): State<PathBuf>,
    Path((id, run, name)): Path<(String, String, String)>,
) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        let path = library::artifact(&root, &id, &run, &name)?;
        let bytes = std::fs::read(path)?;
        let mime = if name.ends_with(".json") {
            "application/json"
        } else if name.ends_with(".csv") {
            "text/csv; charset=utf-8"
        } else {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        };
        Ok::<_, Error>(
            (
                [
                    (header::CONTENT_TYPE, mime),
                    (
                        header::CONTENT_DISPOSITION,
                        &format!("inline; filename=\"{name}\""),
                    ),
                ],
                bytes,
            )
                .into_response(),
        )
    })
    .await;
    match result {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => response(Err(e)),
        Err(e) => response(Err(Error::new(1, "INTERNAL", "http", e))),
    }
}
async fn source(
    State(root): State<PathBuf>,
    Path((id, run, path)): Path<(String, String, String)>,
) -> Response {
    blocking(move || files::read_json(&library::source_file(&root, &id, &run, &path)?)).await
}
