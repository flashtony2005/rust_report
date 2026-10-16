//! 定义 JSON 通道端到端（M7-W W1）：**磁盘上的一个 `.json` 文件**跑通
//! `/render` → `/reports` → `/meta` 三条路由。
//!
//! # 为什么单独立这个文件（不与 `reportdef_http.rs` / `params_http.rs` 重复）
//!
//! 那两份用的是**内联 JSON 字面量**，各验一条路由。本文件验的是**前端 W1
//! 真正依赖的那条链**：同一个文件字节，先渲染、再入库、再按 id 读回参数清单 ——
//! 这正是 `web/src/report.ts` 的 `fetchSnapshot` → `saveReport` → `fetchMeta`
//! 在服务端的镜像。判据形态与 `docs/spec-m7-web.md` §4 W1 相同（真 socket + 真
//! HTTP + 真文件字节）。
//!
//! # 夹具
//!
//! `tests/fixtures/def_params_demo.json` —— 与浏览器端到端（W1 验收）**同一个
//! 文件**：含一条静态参数（带默认值）与一条动态参数，且两个参数都写进单元格
//! 表达式（`@topN` / `@region`），故「参数真的进了渲染结果」可被断言。
//!
//! ⚠ 未登记的 `@参数` 会 `EngineError::UnknownParam`（`expr/atom.rs`），
//! 提交时缺参数 ⇒ 400 —— 这条也被下面的测试钉住（不是静默取 null）。

mod common;

/// 夹具文件名（前端 `accept` 与后端 multipart `filename` 都用它）。
const DEF_FILE: &str = "def_params_demo.json";

/// 取夹具字节（磁盘原文，不经任何字符串往返）。
fn def_bytes() -> Vec<u8> {
    common::fixture(DEF_FILE)
}

/// ① `/render` 收定义 JSON，且参数值真的进了快照。
#[tokio::test]
async fn json_definition_renders_with_param_values() {
    let addr = common::spawn().await;
    // ⚠ 字节串字面量不许含非 ASCII（编译期错）⇒ 中文参数值走 `str::as_bytes()`。
    let params = r#"{"topN":10,"region":"华东"}"#;
    let (body, ct) = common::multipart(&[
        ("report", Some(DEF_FILE), &def_bytes()),
        // 前端 `paramsToObject` 的产物形态：整数走 number、字符串走 string。
        ("params", None, params.as_bytes()),
    ]);
    let resp = common::post_to(&addr, "/render", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());

    let text = resp.body_text();
    // 参数值落进快照：`@topN` ⇒ 10、`@region` ⇒ "华东"。
    assert!(
        text.contains("华东"),
        "动态参数 region 的值应出现在快照里：{}",
        &text[..text.len().min(400)]
    );
}

/// ② 缺参数 ⇒ **显式 400**（未登记的 `@参数` 不静默取 null）。
///
/// 这条钉的是「不猜」边界：参数声明在 JSON 里，但**提交时没给值** ⇒ 报错，
/// 而不是悄悄渲染成空白。
#[tokio::test]
async fn missing_param_value_is_400_not_silently_null() {
    let addr = common::spawn().await;
    // 只给 topN，不给 region（动态参数，无默认值）。
    let (body, ct) = common::multipart(&[
        ("report", Some(DEF_FILE), &def_bytes()),
        ("params", None, br#"{"topN":10}"#),
    ]);
    let resp = common::post_to(&addr, "/render", body, &ct).await;
    assert_eq!(resp.status, 400, "响应体：{}", resp.body_text());
    let text = resp.body_text();
    assert!(
        text.contains("region"),
        "错误消息应点名缺失的参数 region：{text}"
    );
}

/// ③ W1-3：`POST /reports` 入库拿 id（字段名是 `file`，与 `/render` 的
/// `report` 不同名 —— 前端 `saveReport` 靠的就是这个字段名）。
#[tokio::test]
async fn definition_uploads_and_yields_an_id() {
    let addr = common::spawn().await;
    let (body, ct) = common::multipart(&[("file", Some(DEF_FILE), &def_bytes())]);
    let resp = common::post_to(&addr, "/reports", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    let j: serde_json::Value = serde_json::from_str(&resp.body_text()).unwrap();
    let id = j["id"].as_i64().expect("响应应含整数 id");
    assert!(id > 0, "id 应为正整数，实得 {id}");
    // 报表名 = 文件名去扩展名（服务端口径，前端提示文案同源）。
    assert_eq!(j["name"], "def_params_demo");

    // ④ W1-3 的后半：入库后 `/reports` 列表可见。
    let list = common::get(&addr, "/reports").await;
    assert_eq!(list.status, 200, "响应体：{}", list.body_text());
    let items: Vec<serde_json::Value> = serde_json::from_str(&list.body_text()).unwrap();
    assert!(
        items.iter().any(|it| it["id"] == j["id"]),
        "新入库的报表应出现在 /reports 列表里：{}",
        list.body_text()
    );
}

/// ⑤ W1-2：`/meta` 读回参数清单 —— 六个字段、静态/动态两态都在。
///
/// 前端 `metaToRows` 只消费 `name` / `is_static` / `default_val`，故这三项的
/// 取值即前端表单的内容；`param_type` / `data_type` / `description` 一并断言，
/// 免得服务端哪天把字段名改了而前端仍静默取到 undefined。
#[tokio::test]
async fn meta_reads_back_the_declared_params() {
    let addr = common::spawn().await;
    let (body, ct) = common::multipart(&[("file", Some(DEF_FILE), &def_bytes())]);
    let resp = common::post_to(&addr, "/reports", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    let id = serde_json::from_str::<serde_json::Value>(&resp.body_text()).unwrap()["id"]
        .as_i64()
        .unwrap();

    let meta = common::get(&addr, &format!("/reports/{id}/meta")).await;
    assert_eq!(meta.status, 200, "响应体：{}", meta.body_text());
    let items: Vec<serde_json::Value> = serde_json::from_str(&meta.body_text()).unwrap();
    assert_eq!(items.len(), 2, "应读回 2 条参数：{}", meta.body_text());

    // 静态：`is_static=true`，默认值原样读回 ⇒ 前端预填 "10"。
    assert_eq!(items[0]["name"], "topN");
    assert_eq!(items[0]["param_type"], 0, "kind 缺省 = static");
    assert_eq!(items[0]["is_static"], true);
    assert_eq!(items[0]["data_type"], 1, "DT_INT");
    assert_eq!(items[0]["default_val"], "10");
    assert_eq!(items[0]["description"], "取前 N 名");

    // 动态：`kind: "dynamic"` ⇒ param_type=1 ⇒ is_static 派生为 false ⇒
    // 前端标「动态」且不预填值（default_val 为 null）。
    assert_eq!(items[1]["name"], "region");
    assert_eq!(items[1]["param_type"], 1, "PARAM_DYNAMIC");
    assert_eq!(items[1]["is_static"], false);
    assert_eq!(items[1]["data_type"], 11, "DT_STRING");
    assert_eq!(items[1]["default_val"], serde_json::Value::Null);
    assert_eq!(items[1]["description"], "区域（渲染时提供）");
}
