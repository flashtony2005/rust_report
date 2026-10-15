//! rpx-server 集成测试共用脚手架：夹具读取 / multipart 拼装 / 真 socket HTTP。
//!
//! 体例同 `crates/rpx-engine/tests/common/loader.rs`（共享测试件进 `common/`）。
//! 不引 reqwest / tower（守 G5 零新依赖）：手写 HTTP/1.1 客户端（tokio
//! `TcpStream`），`Connection: close` + 读到 EOF。
//!
//! ⛔ 响应体是**原始字节**（`Vec<u8>`）—— M6-03 的 xlsx 是二进制，
//! 不得在任何一层走 lossy 字符串转换（render 侧调用方自行转文本）。
//!
//! dead_code 豁免理由：同一份脚手架被 `render_http.rs` 与 `export_http.rs`
//! 两个独立测试 crate 各自编译，**各自只取所需**（如 render 侧不用 `get`、
//! export 侧不用 `headers`）——「未用项」是分工的常态，不是死代码。

#![allow(dead_code)]

use std::path::PathBuf;

/// multipart 边界（两个测试文件共用同一常量即可：各测试独立连接）。
pub const BOUNDARY: &str = "rpx-e2e-boundary";

/// 读 `tests/fixtures/` 下的夹具（字节原文——GBK 数据集不得被 UTF-8 化）。
pub fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("读夹具失败 {}：{e}", path.display()))
}

/// 拼 multipart 体（parts = (字段名, Option<文件名>, 字节)）。
pub fn multipart(parts: &[(&str, Option<&str>, &[u8])]) -> (Vec<u8>, String) {
    let mut body = Vec::new();
    for (name, filename, bytes) in parts {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        match filename {
            Some(f) => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\n")
                    .as_bytes(),
            ),
            None => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n").as_bytes(),
            ),
        }
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={BOUNDARY}"))
}

/// 一次 HTTP 响应：状态码 + 头（原名原值）+ **原始**体字节。
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// 取头（名不区分大小写；axum 输出小写）。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// 体转文本（仅限文本响应；二进制请直接用 [`Self::body`]）。
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// multipart POST 到指定路径。
pub async fn post_to(addr: &str, path: &str, body: Vec<u8>, content_type: &str) -> HttpResponse {
    send(addr, "POST", path, Some((content_type, body))).await
}

/// 裸 GET（不带 body）。
pub async fn get(addr: &str, path: &str) -> HttpResponse {
    send(addr, "GET", path, None).await
}

/// 真 socket 请求（`Connection: close` + 读到 EOF）→ 拆解响应。
async fn send(addr: &str, method: &str, path: &str, body: Option<(&str, Vec<u8>)>) -> HttpResponse {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let head = match &body {
        Some((ct, b)) => format!(
            "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            b.len()
        ),
        None => format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    };
    stream.write_all(head.as_bytes()).await.unwrap();
    if let Some((_, b)) = body {
        stream.write_all(&b).await.unwrap();
    }
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).await.unwrap();
    parse_response(&resp)
}

/// 拆 HTTP/1.1 响应：状态行 + 头 + 空行 + **原始体**。
fn parse_response(resp: &[u8]) -> HttpResponse {
    let split = resp
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(resp.len());
    let head = String::from_utf8_lossy(&resp[..split]);
    let body = resp.get(split + 4..).unwrap_or_default().to_vec();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    HttpResponse {
        status,
        headers,
        body,
    }
}

/// 起服务（ephemeral 端口），返回 `127.0.0.1:<port>`。
/// 资产库用内存库——每个测试独享，互不串数据；无数据源注册表
/// （HTTP 面这些夹具全是文件数据集模板）。
pub async fn spawn() -> String {
    let store = std::sync::Arc::new(rpx_ds::AssetStore::open(":memory:").await.unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, rpx_server::http::app(store, None))
            .await
            .unwrap();
    });
    format!("127.0.0.1:{port}")
}
