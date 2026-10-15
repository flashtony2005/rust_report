//! 参数声明端到端（M7-4）：**JSON `params[]` → `/reports` 入库 → `/meta` 读回**。
//!
//! 真 socket + 真 multipart 字节（脚手架同 `reportdef_http.rs`）。
//!
//! # 判据：一条链同时压**解析侧与写侧**
//!
//! ```text
//! JSON params[] ──parse──▶ ReportDefine.params
//!      ──rpx_core::to_bytes──▶ .rpx 字节        （M7-2 写侧）
//!      ──AssetStore::put──▶ open_bytes ──▶ param_entries
//!      ──▶ report_params 表 ──▶ GET /reports/{id}/meta
//! ```
//!
//! 参数清单要原样读回，就**必须**先正确写进 `.rpx` 字节、再正确读出来 ——
//! 因此 `/meta` 的断言同时是「`params[]` 解析正确」与「`write_param_meta` 正确」
//! 的证据。这一步不依赖任何生产侧的中间产物，全部从 HTTP 字节读。
//!
//! # 与 `render_http.rs::asset_meta_lists_params` 的分工
//!
//! 那份走 `full.rpx`（**Java 侧**夹具声明的参数），验的是**读侧**；
//! 本文件走定义 JSON（**本契约**声明的参数），验的是**写侧 + 读回**。两条互补。

mod common;

/// 带三条参数声明的定义 JSON（M7-4）：静态带默认值/描述、静态无描述、动态无默认值。
///
/// 三条覆盖 `/meta` 全部六个输出字段的**存在与取值**：
/// `name` / `param_type` / `is_static` / `data_type` / `default_val` / `description`。
const DEF_PARAMS: &str = r#"{
  "version": 1,
  "grid": { "rows": 2, "cols": 2 },
  "cells": [ { "row": 1, "col": 1, "text": "参数化报表" } ],
  "params": [
    { "name": "startDate", "data_type": "string",
      "default": "2024-01-01", "description": "开始日期" },
    { "name": "topN", "data_type": "int", "default": "10" },
    { "name": "region", "data_type": "string", "kind": "dynamic" }
  ]
}"#;

/// 上传 → `/meta`：参数清单六个字段逐条读回。
#[tokio::test]
async fn param_declaration_reaches_the_meta_endpoint() {
    let addr = common::spawn().await;
    let (body, ct) = common::multipart(&[("file", Some("def_params.json"), DEF_PARAMS.as_bytes())]);
    let resp = common::post_to(&addr, "/reports", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    let j: serde_json::Value = serde_json::from_str(&resp.body_text()).unwrap();
    let id = j["id"].as_i64().expect("id 应为整数");
    assert_eq!(j["name"], "def_params", "报表名 = 文件名去扩展名");

    let meta = common::get(&addr, &format!("/reports/{id}/meta")).await;
    assert_eq!(meta.status, 200, "响应体：{}", meta.body_text());
    let items: Vec<serde_json::Value> =
        serde_json::from_str(&meta.body_text()).expect("/meta 应返回 JSON 数组");
    assert_eq!(items.len(), 3, "应读回 3 条参数：{}", meta.body_text());

    // ① 静态 + DT_STRING(11) + 默认值 + 描述 —— 六个字段全给。
    assert_eq!(items[0]["name"], "startDate");
    assert_eq!(
        items[0]["param_type"], 0,
        "kind 缺省 = static（PARAM_NORMAL）"
    );
    assert_eq!(items[0]["is_static"], true);
    assert_eq!(items[0]["data_type"], 11, "DT_STRING");
    assert_eq!(items[0]["default_val"], "2024-01-01");
    assert_eq!(items[0]["description"], "开始日期");

    // ② 静态 + DT_INT(1)：描述缺省 ⇒ `null`（不是空串、不是字段消失）。
    assert_eq!(items[1]["name"], "topN");
    assert_eq!(items[1]["param_type"], 0);
    assert_eq!(items[1]["is_static"], true);
    assert_eq!(items[1]["data_type"], 1, "DT_INT");
    assert_eq!(items[1]["default_val"], "10");
    assert_eq!(items[1]["description"], serde_json::Value::Null);

    // ③ 动态：`kind: "dynamic"` ⇒ param_type=1 ⇒ `is_static` 派生为 false。
    assert_eq!(items[2]["name"], "region");
    assert_eq!(items[2]["param_type"], 1, "PARAM_DYNAMIC");
    assert_eq!(items[2]["is_static"], false);
    assert_eq!(items[2]["data_type"], 11, "DT_STRING");
    assert_eq!(items[2]["default_val"], serde_json::Value::Null);
}

/// 不声明 `params` ⇒ 空清单（且仍是 200：库里的字节是合法 `.rpx`）。
#[tokio::test]
async fn definition_without_params_has_empty_manifest() {
    let addr = common::spawn().await;
    let def = br#"{"version":1,"grid":{"rows":1,"cols":1},"cells":[]}"#;
    let (body, ct) = common::multipart(&[("file", Some("plain.json"), def)]);
    let resp = common::post_to(&addr, "/reports", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    let id: i64 = serde_json::from_str::<serde_json::Value>(&resp.body_text()).unwrap()["id"]
        .as_i64()
        .unwrap();

    let meta = common::get(&addr, &format!("/reports/{id}/meta")).await;
    assert_eq!(meta.status, 200, "响应体：{}", meta.body_text());
    assert_eq!(meta.body_text(), "[]", "不声明参数 ⇒ 空清单");
}

/// 非法参数声明 → 400 + JSON 路径，且**不落库**（与 `cells` 同一套错误面）。
#[tokio::test]
async fn bad_param_declaration_is_400_with_json_path() {
    let addr = common::spawn().await;
    for (params, want) in [
        // `name` 必填（`param_entry` 会静默丢无名参数 ⇒ 必须先拦）。
        (r#"[{"data_type":"string"}]"#, "$.params[0].name"),
        // 未知 `data_type`（`decimal` 不是 `DT_*` 的枚举名）。
        (
            r#"[{"name":"p","data_type":"decimal"}]"#,
            "$.params[0].data_type",
        ),
        // `default` 必须是字符串（`.rpx` 里参数值是 String）。
        (
            r#"[{"name":"p","data_type":"int","default":10}]"#,
            "$.params[0].default",
        ),
        // 未知 `kind`。
        (
            r#"[{"name":"p","data_type":"string","kind":"weird"}]"#,
            "$.params[0].kind",
        ),
        // `params` 本身不是数组。
        (r#"{"a":1}"#, "$.params"),
    ] {
        let def = format!(r#"{{"version":1,"grid":{{"rows":1,"cols":1}},"params":{params}}}"#);
        let (body, ct) = common::multipart(&[("file", Some("bad.json"), def.as_bytes())]);
        let resp = common::post_to(&addr, "/reports", body, &ct).await;
        assert_eq!(resp.status, 400, "响应体：{}", resp.body_text());
        assert!(
            resp.body_text().contains("报表定义不合法"),
            "响应体：{}",
            resp.body_text()
        );
        assert!(
            resp.body_text().contains(want),
            "错误消息应指向 {want}，实得：{}",
            resp.body_text()
        );
    }
    // 拒收即不落库（5 条全拒 ⇒ 库里一条都没有）。
    let list = common::get(&addr, "/reports").await;
    assert_eq!(
        list.body_text(),
        "[]",
        "非法声明不得入库：{}",
        list.body_text()
    );
}
