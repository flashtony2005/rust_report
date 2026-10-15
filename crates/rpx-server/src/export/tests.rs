//! 导出面单元钉子：`fmt` 解析严格性 + 响应头（双形态文件名 + 注入消毒）。

use axum::http::header;
use axum::http::StatusCode;

use super::{ascii_filename, export_response, rfc5987_encode, ExportFormat};

#[test]
fn fmt_parse_is_strict_lowercase() {
    assert_eq!(ExportFormat::parse("csv"), Some(ExportFormat::Csv));
    assert_eq!(ExportFormat::parse("xlsx"), Some(ExportFormat::Xlsx));
    // 严格小写：大小写变体 / 未知值 / 空串都不接受（调用方报 400，不静默回落）。
    assert_eq!(ExportFormat::parse("CSV"), None);
    assert_eq!(ExportFormat::parse("Xlsx"), None);
    assert_eq!(ExportFormat::parse("pdf"), None);
    assert_eq!(ExportFormat::parse(""), None);
}

#[test]
fn ascii_filename_keeps_whitelist_and_falls_back() {
    assert_eq!(ascii_filename("engine_sum-2.0"), "engine_sum-2.0");
    // 非白名单字符逐字换 `_`（空格也换）——保数字尾缀可用于识别。
    assert_eq!(ascii_filename("月报 2026"), "___2026");
    // 全为 `_`/`.` 或空 ⇒ 兜底名。
    assert_eq!(ascii_filename(""), "export");
    assert_eq!(ascii_filename("月报"), "export");
    assert_eq!(ascii_filename("..."), "export");
}

#[test]
fn rfc5987_encodes_non_attr_chars() {
    // 中文四字 + 空格 + 引号：全部落 %XX（attr-char 集外）。
    assert_eq!(
        rfc5987_encode("月度报表"),
        "%E6%9C%88%E5%BA%A6%E6%8A%A5%E8%A1%A8"
    );
    assert_eq!(rfc5987_encode("a b\"c"), "a%20b%22c");
    assert_eq!(rfc5987_encode("ok-._~"), "ok-._~"); // attr-char 保留
}

#[tokio::test]
async fn response_is_complete_for_ascii_name() {
    let resp = export_response(b"a,b\r\n".to_vec(), "engine_sum", ExportFormat::Csv);
    assert_eq!(resp.status(), StatusCode::OK);
    let (parts, body) = resp.into_parts();
    assert_eq!(
        parts.headers[header::CONTENT_TYPE],
        "text/csv; charset=utf-8"
    );
    assert_eq!(
        parts.headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"engine_sum.csv\"; filename*=UTF-8''engine_sum.csv"
    );
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    assert_eq!(&bytes[..], b"a,b\r\n");
}

#[tokio::test]
async fn response_headers_for_utf8_name() {
    let resp = export_response(vec![0x50, 0x4B], "月度报表", ExportFormat::Xlsx);
    let (parts, _) = resp.into_parts();
    assert_eq!(
        parts.headers[header::CONTENT_TYPE],
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
    // ASCII fallback = export；UTF-8 形态是百分号编码（浏览器读取 `filename*`）。
    assert_eq!(
        parts.headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"export.xlsx\"; \
         filename*=UTF-8''%E6%9C%88%E5%BA%A6%E6%8A%A5%E8%A1%A8.xlsx"
    );
}

#[test]
fn response_filename_never_breaks_header_syntax() {
    // 注入面：引号 / CR / LF / 分号 / 冒号。
    let evil = "a\"; b\r\nX-Evil: 1";
    let resp = export_response(Vec::new(), evil, ExportFormat::Csv);
    let value = resp.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap();
    assert!(
        !value.contains('\r') && !value.contains('\n'),
        "无 CR/LF：{value}"
    );
    assert_eq!(
        value.matches('"').count(),
        2,
        "只有包裹 ASCII 名的两个引号：{value}"
    );
    // 非 ASCII 注入形态同样只是被编码的数据（例如引号 → %22）。
    assert!(!value.contains(evil));
}
