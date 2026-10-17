use rpx_runtime::{files, library, run};
use serde_json::{json, Value as J};
use std::{fs, path::PathBuf};
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
async fn send(
    addr: &str,
    method: &str,
    path: &str,
    value: Option<&J>,
    revision: Option<&str>,
) -> (u16, Vec<u8>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let body = value
        .map(|v| serde_json::to_vec(v).unwrap())
        .unwrap_or_default();
    let revision = revision
        .map(|v| format!("If-Match: {v}\r\n"))
        .unwrap_or_default();
    let head=format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\n{revision}Content-Length: {}\r\n\r\n",body.len());
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(&body).await.unwrap();
    let mut output = Vec::new();
    stream.read_to_end(&mut output).await.unwrap();
    let split = output.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let status = String::from_utf8_lossy(&output[..split])
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    (status, output[split + 4..].to_vec())
}
#[tokio::test]
async fn web_saves_json_with_revision_and_reads_fixed_artifacts() {
    let temp = Temp(std::env::temp_dir().join(format!("rpx-http-{}", files::unique())));
    let root = &temp.0;
    library::init(root, true).unwrap();
    let original = library::show(root, "complex-master").unwrap();
    let revision = original["revision"].as_str().unwrap();
    let success = library::execute(
        root,
        "complex-master",
        revision,
        &run::Options {
            exports: vec!["csv".into()],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Rendering remains readable with all business inputs removed.
    fs::remove_file(root.join("complex-master/data/complex-master-sales.csv")).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let app = rpx_server::http::app(root.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let (status, bytes) = send(&addr, "GET", "/reports/complex-master", None, None).await;
    assert_eq!(status, 200);
    let mut detail: J = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(detail["revision"], revision);
    detail["template"]["cells"][0]["text"] = json!("人工修正");
    let path = "/reports/complex-master/template";
    assert_eq!(
        send(&addr, "PUT", path, Some(&detail["template"]), None)
            .await
            .0,
        428
    );
    assert_eq!(
        send(
            &addr,
            "PUT",
            path,
            Some(&detail["template"]),
            Some(revision)
        )
        .await
        .0,
        200
    );
    assert_eq!(
        send(
            &addr,
            "PUT",
            path,
            Some(&detail["template"]),
            Some(revision)
        )
        .await
        .0,
        412
    );
    let (status, bytes) = send(&addr, "GET", "/reports/complex-master", None, None).await;
    assert_eq!(status, 200);
    let state: J = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state["result_state"], "older_revision");
    let run = success["run"]["run_id"].as_str().unwrap();
    let source = format!("/reports/complex-master/runs/{run}/source/template.json");
    let (status, bytes) = send(&addr, "GET", &source, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_slice::<J>(&bytes).unwrap(),
        original["template"]
    );
    assert_eq!(
        send(
            &addr,
            "GET",
            &source.replace("template.json", "missing.json"),
            None,
            None
        )
        .await
        .0,
        404
    );
    let artifact = format!("/reports/complex-master/runs/{run}/artifacts/workbook.json");
    let (status, bytes) = send(&addr, "GET", &artifact, None, None).await;
    assert_eq!(status, 200);
    assert_eq!(
        bytes,
        fs::read(library::artifact(root, "complex-master", run, "workbook.json").unwrap()).unwrap()
    );
    assert_eq!(
        send(&addr, "POST", "/render", Some(&json!({})), None)
            .await
            .0,
        404
    );
    assert_eq!(
        send(&addr, "POST", "/export", Some(&json!({})), None)
            .await
            .0,
        404
    );
    assert_eq!(library::runs(root, "complex-master").unwrap().len(), 1);
    task.abort();
}

async fn send_raw(addr: &str, path: &str, content_type: &str, body: &[u8]) -> (u16, Vec<u8>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let head = format!(
        "PUT {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: {content_type}\r\nIf-Match: dummy\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    let mut output = Vec::new();
    stream.read_to_end(&mut output).await.unwrap();
    let split = output.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let status = String::from_utf8_lossy(&output[..split])
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    (status, output[split + 4..].to_vec())
}

#[tokio::test]
async fn json_rejections_are_json_errors() {
    let temp = Temp(std::env::temp_dir().join(format!("rpx-http-json-{}", files::unique())));
    library::init(&temp.0, false).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let app = rpx_server::http::app(temp.0.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let (status, bytes) = send_raw(&addr, "/reports/x/template", "application/json", b"{bad").await;
    assert_eq!(status, 400);
    let value: J = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["error"]["code"], "JSON_INVALID");

    let (status, bytes) = send_raw(
        &addr,
        "/reports/x/template",
        "text/plain",
        br#"{"grid":{"rows":1,"cols":1}}"#,
    )
    .await;
    assert_eq!(status, 415);
    let value: J = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["error"]["code"], "UNSUPPORTED_MEDIA_TYPE");

    task.abort();
}
