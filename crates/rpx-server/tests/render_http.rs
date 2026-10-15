//! `POST /render` 端到端（M3-05）：**真 socket + 真 multipart 字节**。
//!
//! 不引 reqwest / tower（守 G5 零新依赖）：手写 HTTP/1.1 客户端在
//! [`common`]（M6-03 起与 `export_http.rs` 共用）。断言口径
//! （`docs/spec-m3.md` §4 M3-05）：响应体与**库函数直调**
//! （[`rpx_server::http::render_request`]）逐字节一致 —— 同输入下 HTTP 面
//! 不得与引擎库输出有差；另覆盖 GBK 字节过网、参数过网、缺文件 / 坏魔数 /
//! 未知字段四条负路径。

mod common;

use rpx_engine::params::ParamTable;
use rpx_engine::value::Value;
use rpx_server::form::RenderRequest;
use rpx_server::http::render_request;

use common::{fixture, multipart, spawn};

/// 文本响应的 POST（render 侧全文本断言；二进制响应走 `common::post_to` 直取字节）。
async fn post(addr: &str, body: Vec<u8>, content_type: &str) -> (u16, String) {
    let r = common::post_to(addr, "/render", body, content_type).await;
    (r.status, r.body_text())
}

/// 文本响应的 multipart POST 到指定路径（M3-04 的 `/reports` 上传复用）。
async fn post_to(addr: &str, path: &str, body: Vec<u8>, content_type: &str) -> (u16, String) {
    let r = common::post_to(addr, path, body, content_type).await;
    (r.status, r.body_text())
}

/// 库函数直调（对拍基准；`csv_files` = 全部 CSV **完整文件名** —— 第二个
/// 数据集在夹具里就叫 `engine_sum.csv2`，不是 `engine_sum2.csv`）。
fn library(name: &str, csv_files: &[&str], params: ParamTable) -> String {
    let req = RenderRequest {
        report: fixture(&format!("{name}.rpx")),
        report_name: name.to_string(),
        datasets: csv_files
            .iter()
            .map(|f| (f.to_string(), fixture(f)))
            .collect(),
        params,
    };
    render_request(&req).unwrap()
}

/// 快照 JSON 的 sheet 段（键集断言用）。
fn sheet_of(json_text: &str) -> serde_json::Value {
    let j: serde_json::Value = serde_json::from_str(json_text).unwrap();
    j["sheets"]["rpx-sheet-1"].clone()
}

#[tokio::test]
async fn render_http_matches_library_output() {
    let addr = spawn().await;
    let (body, ct) = multipart(&[
        ("report", Some("engine_sum.rpx"), &fixture("engine_sum.rpx")),
        (
            "dataset",
            Some("engine_sum.csv"),
            &fixture("engine_sum.csv"),
        ),
        (
            "dataset",
            Some("engine_sum.csv2"),
            &fixture("engine_sum.csv2"),
        ),
    ]);
    let (status, resp) = post(&addr, body, &ct).await;
    assert_eq!(status, 200, "响应体：{resp}");
    // 同输入下 HTTP 面与库函数直调**逐字节一致**。
    assert_eq!(
        resp,
        library(
            "engine_sum",
            &["engine_sum.csv", "engine_sum.csv2"],
            ParamTable::EMPTY
        )
    );
    // 键集（契约 `docs/spec-m4.md` §4）：IWorkbookData 顶层 + sheet 槽。
    let j: serde_json::Value = serde_json::from_str(&resp).unwrap();
    for key in [
        "id",
        "name",
        "appVersion",
        "locale",
        "styles",
        "sheetOrder",
        "sheets",
    ] {
        assert!(j.get(key).is_some(), "缺顶层键 {key}");
    }
    for key in ["cellData", "mergeData", "rowData", "columnData"] {
        assert!(sheet_of(&resp).get(key).is_some(), "缺 sheet 键 {key}");
    }
}

#[tokio::test]
async fn pagination_block_is_real_caliber_with_page_ranges() {
    // M3-06：`/render` 的 `pagination` = **模板真值口径**（纸码表 + 模板边距），
    // 不是探针的 595×842。契约 `docs/spec-m4.md` §4.5（rpx 扩展键，workbook 顶层）。
    let addr = spawn().await;
    let (body, ct) = multipart(&[
        ("report", Some("engine_gbk.rpx"), &fixture("engine_gbk.rpx")),
        (
            "dataset",
            Some("engine_gbk.csv"),
            &fixture("engine_gbk.csv"),
        ),
    ]);
    let (status, resp) = post(&addr, body, &ct).await;
    assert_eq!(status, 200, "响应体：{resp}");
    let j: serde_json::Value = serde_json::from_str(&resp).unwrap();
    let pg = &j["pagination"];
    // 默认 setup（paper=9 A4、边距 19/19/25/25）⇒ 可用 172×247mm ⇒ pt。
    // Pagination 的 paper_w/h 是 f32 存储 ⇒ 与 f64 真值比到 f32 精度。
    let expect_w = (172.0 * 72.0 / 25.4) as f32;
    let expect_h = (247.0 * 72.0 / 25.4) as f32;
    let got_w = pg["page_w_pt"].as_f64().unwrap() as f32;
    let got_h = pg["page_h_pt"].as_f64().unwrap() as f32;
    assert!(
        (got_w - expect_w).abs() < 1e-3,
        "page_w_pt 应为 A4 减边距的真值 {expect_w}，实得 {got_w}"
    );
    assert!(
        (got_h - expect_h).abs() < 1e-3,
        "page_h_pt 应为 A4 减边距的真值 {expect_h}，实得 {got_h}"
    );
    // 显然不是探针口径（595×842 零边距）。
    assert!(
        (got_w - 595.0).abs() > 1.0 && (got_h - 842.0).abs() > 1.0,
        "pagination 不应是探针口径"
    );
    // 键集与页内结构；row_range/col_range 为 0 起坐标。
    for key in [
        "page_w_pt",
        "page_h_pt",
        "columns",
        "page_count",
        "x_pages",
        "y_pages",
        "row_num_per_page",
        "pages",
    ] {
        assert!(pg.get(key).is_some(), "缺 pagination 键 {key}");
    }
    let page1 = &pg["pages"][0];
    assert_eq!(page1["page"], 1);
    assert!(page1["rows"].as_i64().unwrap() > 0);
    let rr = page1["row_range"].as_array().unwrap();
    let cr = page1["col_range"].as_array().unwrap();
    assert_eq!(rr.len(), 2);
    assert_eq!(cr.len(), 2);
    assert!(rr[0].as_i64().unwrap() >= 0, "row_range 应为 0 起");
    assert!(cr[0].as_i64().unwrap() >= 0, "col_range 应为 0 起");
}

#[tokio::test]
async fn engine_error_maps_to_400() {
    // 引擎错误 → 400 的映射（RenderError::Engine 臂）：engine_param.rpx 的表达式
    // 用 `@p`（M3-01），不带 params ⇒ 建树期 `UnknownParam`。真值口径守卫
    // （UnsupportedPaging）走同一映射臂，行为已在引擎侧钉死（pager_guard.rs）。
    let addr = spawn().await;
    let (body, ct) = multipart(&[
        (
            "report",
            Some("engine_param.rpx"),
            &fixture("engine_param.rpx"),
        ),
        (
            "dataset",
            Some("engine_param.csv"),
            &fixture("engine_param.csv"),
        ),
    ]);
    let (status, resp) = post(&addr, body, &ct).await;
    assert_eq!(status, 400, "响应体：{resp}");
    assert!(resp.contains("\"error\""), "错误体应为 JSON，实得：{resp}");
    assert!(resp.contains("@"), "错误消息应含未登记参数名，实得：{resp}");
}

#[tokio::test]
async fn gbk_csv_decodes_over_http() {
    // M3-02 × M3-05：GBK 字节过 multipart 不得被 UTF-8 化（静默乱码回归网）。
    let addr = spawn().await;
    let (body, ct) = multipart(&[
        ("report", Some("engine_gbk.rpx"), &fixture("engine_gbk.rpx")),
        (
            "dataset",
            Some("engine_gbk.csv"),
            &fixture("engine_gbk.csv"),
        ),
    ]);
    let (status, resp) = post(&addr, body, &ct).await;
    assert_eq!(status, 200, "响应体：{resp}");
    assert_eq!(
        resp,
        library("engine_gbk", &["engine_gbk.csv"], ParamTable::EMPTY)
    );
    for s in ["区域", "华北", "华南", "华东"] {
        assert!(resp.contains(s), "GBK 中文 {s} 应正确解码进快照");
    }
}

#[tokio::test]
async fn params_flow_over_http() {
    // M3-01 × M3-05：params JSON → 引擎计算；List 参数驱动扩展（矩阵 3→5 行）。
    let addr = spawn().await;
    let (body, ct) = multipart(&[
        (
            "report",
            Some("engine_param.rpx"),
            &fixture("engine_param.rpx"),
        ),
        (
            "dataset",
            Some("engine_param.csv"),
            &fixture("engine_param.csv"),
        ),
        ("params", None, br#"{"p":[1,2,3],"s":7}"#),
    ]);
    let (status, resp) = post(&addr, body, &ct).await;
    assert_eq!(status, 200, "响应体：{resp}");
    let mut params = ParamTable::new();
    params.insert(
        "p",
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
    );
    params.insert("s", Value::Int(7));
    assert_eq!(resp, library("engine_param", &["engine_param.csv"], params));
    // List 参数把矩阵扩到第 5 行 ⇒ cellData 应有 0 起行键 "4"（`@s` 落该行）。
    let cd = sheet_of(&resp)["cellData"].clone();
    assert!(
        cd.get("4").is_some(),
        "List 参数应把矩阵扩到第 5 行，实得 cellData 行键：{:?}",
        cd.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
}

#[tokio::test]
async fn missing_dataset_file_is_400() {
    // 只传 .rpx 不传 CSV ⇒ 400，且消息点名**文件名**（不是数据集名）。
    let addr = spawn().await;
    let (body, ct) = multipart(&[("report", Some("engine_sum.rpx"), &fixture("engine_sum.rpx"))]);
    let (status, resp) = post(&addr, body, &ct).await;
    assert_eq!(status, 400, "响应体：{resp}");
    assert!(
        resp.contains("engine_sum.csv"),
        "应点名缺失文件，实得：{resp}"
    );
}

#[tokio::test]
async fn bad_magic_is_400() {
    let addr = spawn().await;
    let (body, ct) = multipart(&[("report", Some("junk.rpx"), b"not an rpx file at all")]);
    let (status, resp) = post(&addr, body, &ct).await;
    assert_eq!(status, 400, "响应体：{resp}");
    assert!(resp.contains("\"error\""), "错误体应为 JSON，实得：{resp}");
}

#[tokio::test]
async fn unknown_form_field_is_400() {
    let addr = spawn().await;
    let (body, ct) = multipart(&[
        ("report", Some("engine_sum.rpx"), &fixture("engine_sum.rpx")),
        ("evil", Some("x.txt"), b"x"),
    ]);
    let (status, resp) = post(&addr, body, &ct).await;
    assert_eq!(status, 400, "响应体：{resp}");
    assert!(resp.contains("未知字段"), "实得：{resp}");
}

// ---- M3-04 资产库三路由（`docs/spec-m3.md` §4 M3-04）----

/// 上传 → 列表 → meta 全链路：`full.rpx` 声明两个**静态**参数
/// （`Fixtures.java:261-266`：startDate DT_STRING 默认 "2024-01-01"、
/// topN DT_INT 默认 "10"）。meta 是**读库即答**——响应必须含
/// 静态/动态分组语义（`is_static`）与默认值，供前端生成参数表单。
#[tokio::test]
async fn asset_upload_list_meta_round_trip() {
    let addr = spawn().await;
    let (body, ct) = multipart(&[("file", Some("full.rpx"), &fixture("full.rpx"))]);
    let (status, resp) = post_to(&addr, "/reports", body, &ct).await;
    assert_eq!(status, 200, "响应体：{resp}");
    let j: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(j["name"], "full", "报表名 = 文件名去扩展名");
    let id = j["id"].as_i64().expect("id 应为整数");

    // 列表含该条目。
    let list_body = raw_get(&addr, "/reports").await;
    let list: serde_json::Value = serde_json::from_str(&list_body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["name"], "full");

    // meta：静态参数清单 + 默认值（Types2.DT_STRING=11 / DT_INT=1，Types2.java:22/:32）。
    let meta_body = raw_get(&addr, &format!("/reports/{id}/meta")).await;
    let meta: serde_json::Value = serde_json::from_str(&meta_body).unwrap();
    let items = meta.as_array().unwrap();
    assert_eq!(items.len(), 2, "{meta_body}");
    assert_eq!(items[0]["name"], "startDate");
    assert_eq!(items[0]["is_static"], true);
    assert_eq!(items[0]["param_type"], 0);
    assert_eq!(items[0]["data_type"], 11);
    assert_eq!(items[0]["default_val"], "2024-01-01");
    assert_eq!(items[1]["name"], "topN");
    assert_eq!(items[1]["is_static"], true);
    assert_eq!(items[1]["data_type"], 1);
    assert_eq!(items[1]["default_val"], "10");
}

/// 坏输入：既非 `.rpx`（缺 `RQQR` 魔数）也非 JSON 定义。
///
/// ⚠ 口径随 M7-2 变更：入库前先过 [`normalize_to_rpx`]，它对「两者都不是」
/// 走 `open_report` 的 `UnknownFormat` 分支，报**无法识别的报表格式**
/// （与 `/render` 同一套错误文本）；旧文本「模板解析失败」只保留给**带 `RQQR`
/// 魔数但正文坏**的 `.rpx`（那条仍由 `AssetStore::put` 的预检产出）。
#[tokio::test]
async fn asset_bad_magic_is_400() {
    let addr = spawn().await;
    let (body, ct) = multipart(&[("file", Some("junk.rpx"), b"not an rpx file at all")]);
    let (status, resp) = post_to(&addr, "/reports", body, &ct).await;
    assert_eq!(status, 400, "响应体：{resp}");
    assert!(resp.contains("无法识别的报表格式"), "实得：{resp}");
    // 拒收即不落库。
    let list_body = raw_get(&addr, "/reports").await;
    assert_eq!(list_body, "[]", "坏魔数不得入库：{list_body}");
}

/// 不存在的 id：meta 显式 404（与空清单可区分）。
#[tokio::test]
async fn asset_meta_missing_id_is_404() {
    let addr = spawn().await;
    let resp = raw_get(&addr, "/reports/999/meta").await;
    assert!(resp.contains("报表不存在"), "实得：{resp}");
    let status = raw_get_status(&addr, "/reports/999/meta").await;
    assert_eq!(status, 404);
}

/// 裸 GET（不带 body；`post` 是 multipart 专用）。
async fn raw_get(addr: &str, path: &str) -> String {
    raw_get_status_body(addr, path).await.1
}

async fn raw_get_status(addr: &str, path: &str) -> u16 {
    raw_get_status_body(addr, path).await.0
}

async fn raw_get_status_body(addr: &str, path: &str) -> (u16, String) {
    let r = common::get(addr, path).await;
    (r.status, r.body_text())
}
