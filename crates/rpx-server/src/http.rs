//! Library JSON source editing and fixed run artifacts. No calculation handlers.
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
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

pub fn app(root: PathBuf) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/reports", get(list))
        .route("/reports/{id}", get(show))
        .route("/reports/{id}/template", axum::routing::put(save))
        .route("/reports/{id}/runs", get(runs))
        .route("/reports/{id}/runs/{run}", get(record))
        .route("/reports/{id}/runs/{run}/source/{*path}", get(source))
        .route("/reports/{id}/runs/{run}/artifacts/{name}", get(artifact))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .with_state(root)
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
async fn save(
    State(root): State<PathBuf>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(value): Json<J>,
) -> Response {
    let expected = headers
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
