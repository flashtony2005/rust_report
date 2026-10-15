//! `POST /export?fmt=csv|xlsx` 端到端（M6-03）：真 socket + 真 multipart 字节。
//!
//! 断言口径（`docs/spec-m6.md` §3 M6-03）：响应头 `Content-Disposition`
//! 带文件名 + body 是合法文件字节。**合法性证据 = 与库函数直调
//! （[`rpx_server::export::export_request`]）逐字节一致**（同一代码路径）；
//! xlsx 载体的合法性（zip 容器 + `calamine` 读回自洽）已在 rpx-export 侧
//! 全量钉死（M6-02，`crates/rpx-export/src/xlsx/tests.rs`），此处不重复构建。
//!
//! 另覆盖：`fmt` 缺省 = xlsx / 非法值 400 / 中文报表名的双形态文件名过网。

mod common;

use rpx_engine::params::ParamTable;
use rpx_server::export::{export_request, ExportFormat};
use rpx_server::form::RenderRequest;

/// 库函数直调的输入（对拍基准，口径同 `render_http.rs::library`）。
fn request(name: &str, csv_files: &[&str]) -> RenderRequest {
    RenderRequest {
        report: common::fixture(&format!("{name}.rpx")),
        report_name: name.to_string(),
        datasets: csv_files
            .iter()
            .map(|f| (f.to_string(), common::fixture(f)))
            .collect(),
        params: ParamTable::EMPTY,
    }
}

/// engine_sum 表单（两数据集模板；render_http 同款）。
fn engine_sum_form() -> (Vec<u8>, String) {
    common::multipart(&[
        (
            "report",
            Some("engine_sum.rpx"),
            &common::fixture("engine_sum.rpx"),
        ),
        (
            "dataset",
            Some("engine_sum.csv"),
            &common::fixture("engine_sum.csv"),
        ),
        (
            "dataset",
            Some("engine_sum.csv2"),
            &common::fixture("engine_sum.csv2"),
        ),
    ])
}

#[tokio::test]
async fn export_csv_matches_library_and_carries_filename() {
    let addr = common::spawn().await;
    let (body, ct) = engine_sum_form();
    let resp = common::post_to(&addr, "/export?fmt=csv", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    // body 与库函数直调**逐字节一致**。
    let want = export_request(
        &request("engine_sum", &["engine_sum.csv", "engine_sum.csv2"]),
        ExportFormat::Csv,
    )
    .unwrap();
    assert_eq!(resp.body, want);
    // 头：Content-Type + Content-Disposition 双形态（ASCII 名两形态同值）。
    assert_eq!(resp.header("content-type"), Some("text/csv; charset=utf-8"));
    assert_eq!(
        resp.header("content-disposition"),
        Some("attachment; filename=\"engine_sum.csv\"; filename*=UTF-8''engine_sum.csv")
    );
    // 语义抽查：满矩阵表头行在（对拍之外的独立字面量）。
    let text = resp.body_text();
    assert!(
        text.starts_with("region,qty,"),
        "导出内容应以表头开始：{}",
        &text[..60.min(text.len())]
    );
}

#[tokio::test]
async fn export_gbk_csv_decodes_to_utf8_over_http() {
    // M3-02 × M6-03：GBK 数据集字节过 multipart → 引擎 → CSV 导出应是 UTF-8
    // 中文原文（与 /render 的 `gbk_csv_decodes_over_http` 同款回归网）。
    let addr = common::spawn().await;
    let (body, ct) = common::multipart(&[
        (
            "report",
            Some("engine_gbk.rpx"),
            &common::fixture("engine_gbk.rpx"),
        ),
        (
            "dataset",
            Some("engine_gbk.csv"),
            &common::fixture("engine_gbk.csv"),
        ),
    ]);
    let resp = common::post_to(&addr, "/export?fmt=csv", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    let text = resp.body_text();
    for s in ["区域", "华北", "华南", "华东"] {
        assert!(text.contains(s), "GBK 中文 {s} 应正确解码进导出：{text}");
    }
    assert_eq!(
        resp.body,
        export_request(
            &request("engine_gbk", &["engine_gbk.csv"]),
            ExportFormat::Csv
        )
        .unwrap()
    );
}

#[tokio::test]
async fn export_xlsx_matches_library_and_is_zip_container() {
    let addr = common::spawn().await;
    let (body, ct) = engine_sum_form();
    let resp = common::post_to(&addr, "/export?fmt=xlsx", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    // 合法 xlsx 的第一硬证据：zip 容器魔数（"PK\x03\x04"）。
    assert_eq!(&resp.body[..4], b"PK\x03\x04", "xlsx 应是 zip 容器");
    let want = export_request(
        &request("engine_sum", &["engine_sum.csv", "engine_sum.csv2"]),
        ExportFormat::Xlsx,
    )
    .unwrap();
    assert_eq!(resp.body, want);
    assert_eq!(
        resp.header("content-type"),
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
    );
    assert_eq!(
        resp.header("content-disposition"),
        Some("attachment; filename=\"engine_sum.xlsx\"; filename*=UTF-8''engine_sum.xlsx")
    );
}

#[tokio::test]
async fn export_defaults_to_xlsx_without_fmt_query() {
    let addr = common::spawn().await;
    let (body, ct) = engine_sum_form();
    let resp = common::post_to(&addr, "/export", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    assert_eq!(&resp.body[..4], b"PK\x03\x04", "缺省应为 xlsx");
    assert!(
        resp.header("content-disposition")
            .unwrap()
            .contains(".xlsx"),
        "文件名应带 .xlsx"
    );
}

#[tokio::test]
async fn export_bad_fmt_is_400() {
    let addr = common::spawn().await;
    let (body, ct) = engine_sum_form();
    let resp = common::post_to(&addr, "/export?fmt=pdf", body, &ct).await;
    assert_eq!(resp.status, 400);
    let text = resp.body_text();
    assert!(text.contains("不支持的导出格式"), "实得：{text}");
    assert!(text.contains("csv / xlsx"), "错误消息应列可选值：{text}");
}

#[tokio::test]
async fn export_utf8_report_name_gets_dual_filenames() {
    // 中文报表名过 multipart（浏览器同款：header 里直接放 UTF-8 字节）。
    let addr = common::spawn().await;
    let (body, ct) = common::multipart(&[
        (
            "report",
            Some("月度报表.rpx"),
            &common::fixture("engine_sum.rpx"),
        ),
        (
            "dataset",
            Some("engine_sum.csv"),
            &common::fixture("engine_sum.csv"),
        ),
        (
            "dataset",
            Some("engine_sum.csv2"),
            &common::fixture("engine_sum.csv2"),
        ),
    ]);
    let resp = common::post_to(&addr, "/export?fmt=csv", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    assert_eq!(
        resp.header("content-disposition"),
        Some(
            "attachment; filename=\"export.csv\"; \
             filename*=UTF-8''%E6%9C%88%E5%BA%A6%E6%8A%A5%E8%A1%A8.csv"
        )
    );
}

#[tokio::test]
async fn export_missing_dataset_is_400_same_as_render() {
    // 与 /render 共享的「缺文件」负路径：消息点名文件名（不因出口不同而变）。
    let addr = common::spawn().await;
    let (body, ct) = common::multipart(&[(
        "report",
        Some("engine_sum.rpx"),
        &common::fixture("engine_sum.rpx"),
    )]);
    let resp = common::post_to(&addr, "/export?fmt=csv", body, &ct).await;
    assert_eq!(resp.status, 400);
    assert!(
        resp.body_text().contains("engine_sum.csv"),
        "应点名缺失文件：{}",
        resp.body_text()
    );
}
