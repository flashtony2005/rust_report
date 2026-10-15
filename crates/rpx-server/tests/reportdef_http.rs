//! 定义 JSON 端到端（M7-1）：**JSON 定义 → `/render` 快照 / `/export` xlsx**。
//!
//! 真 socket + 真 multipart 字节（脚手架同 `render_http.rs` / `export_http.rs`）。
//!
//! # 判据（三段，互不重叠）
//!
//! 1. **接线等价**：同输入下 HTTP 面与库函数直调
//!    （[`rpx_server::http::render_request`]）**逐字节一致** —— 证明 JSON 定义走的是
//!    与 `.rpx` 完全相同的主链路，没有旁路。
//! 2. **几何真值**：快照 `mergeData` 的锚点矩形与**手写期望**全等（0 起坐标）——
//!    静态跨列合并 + 明细区**同值纵向合并**两类都要出现。这是 `.rpx` 侧对拍之外
//!    的独立字面量（不复用生产代码的中间产物）。
//! 3. **导出读回**：`/export?fmt=xlsx` 的响应体是合法 zip，且 `calamine` 读回的
//!    合并区/取值与**手写期望**全等 —— 从 HTTP 字节反读，不经任何生产侧辅助函数。
//!
//! 负路径另覆盖：JSON 结构错（400 + JSON 路径）/ 两者都不是的字节（400 显式报格式）。
//!
//! # 为什么用 `engine_samemerge.csv`
//!
//! 它是现成的「相邻等值」数据集（`A/A/B/B/C`），正好把**同值合并**逼出来：
//! 期望 2 行一段的 `A`、2 行一段的 `B`、单行 `C`（不并），且 `A`/`B` 两段
//! **跨度相同且上下相邻** —— 即 `rpx-export` 2026-10-16 修掉的那个
//! 「等跨度相邻区互判延续槽」缺陷的触发面（回归网在此再兜一层）。

mod common;

use calamine::{Data, Reader, Xlsx};
use rpx_engine::params::ParamTable;
use rpx_server::form::RenderRequest;
use rpx_server::http::render_request;

/// 定义 JSON：文件数据集 + 跨列标题合并 + 明细行**同值纵向合并** + 合计聚合。
///
/// 矩阵 4 逻辑行 × 2 逻辑列（`Builder` 内部加一，前端口径见 `docs/report-def-json.md`）：
/// 第 1 行标题（跨 2 列）、第 2 行表头、第 3 行明细（`ds1.select` 驱动扩展）、
/// 第 4 行合计（`ds1.sum`）。
const DEF: &str = r#"{
  "version": 1,
  "grid": { "rows": 4, "cols": 2 },
  "cells": [
    { "row": 1, "col": 1, "text": "区域销售明细", "merge": { "rows": 1, "cols": 2 } },
    { "row": 2, "col": 1, "text": "区域", "h_align": "center" },
    { "row": 2, "col": 2, "text": "数量", "h_align": "center" },
    { "row": 3, "col": 1, "expr": "ds1.select(region)", "merge_same": "vertical" },
    { "row": 3, "col": 2, "expr": "ds1.select(qty)" },
    { "row": 4, "col": 1, "text": "合计", "h_align": "right" },
    { "row": 4, "col": 2, "expr": "ds1.sum(qty)" }
  ],
  "row_props": [ { "row": 1, "height": 30 } ],
  "col_props": [ { "col": 1, "width": 20 }, { "col": 2, "width": 20 } ],
  "datasets": [
    { "name": "ds1", "file": { "file_name": "engine_samemerge.csv", "encoding": "UTF-8" } }
  ]
}"#;

/// 数据集文件名（必须与 `DEF` 的 `file_name` 逐字一致 —— 引擎按它取数）。
const CSV: &str = "engine_samemerge.csv";

/// 表单（`report` = JSON 定义字节，`dataset` = CSV）。
fn form() -> (Vec<u8>, String) {
    common::multipart(&[
        ("report", Some("def.json"), DEF.as_bytes()),
        ("dataset", Some(CSV), &common::fixture(CSV)),
    ])
}

/// 库函数直调（对拍基准）。
fn library() -> String {
    let req = RenderRequest {
        report: DEF.as_bytes().to_vec(),
        report_name: "def".to_string(),
        datasets: [(CSV.to_string(), common::fixture(CSV))]
            .into_iter()
            .collect(),
        params: ParamTable::EMPTY,
    };
    render_request(&req).unwrap()
}

/// 快照 sheet 段。
fn sheet(json_text: &str) -> serde_json::Value {
    let j: serde_json::Value = serde_json::from_str(json_text).unwrap();
    j["sheets"]["rpx-sheet-1"].clone()
}

/// `mergeData` → `(startRow, startColumn, endRow, endColumn)` 列表（0 起）。
fn merge_rects(sheet: &serde_json::Value) -> Vec<(i64, i64, i64, i64)> {
    sheet["mergeData"]
        .as_array()
        .expect("mergeData 应是数组")
        .iter()
        .map(|m| {
            (
                m["startRow"].as_i64().unwrap(),
                m["startColumn"].as_i64().unwrap(),
                m["endRow"].as_i64().unwrap(),
                m["endColumn"].as_i64().unwrap(),
            )
        })
        .collect()
}

/// `cellData[r][c].v`（0 起坐标）。
fn cell_v(sheet: &serde_json::Value, r: usize, c: usize) -> serde_json::Value {
    sheet["cellData"][r.to_string()][c.to_string()]["v"].clone()
}

/// 合并矩形（0 起闭区间）：`((r0,c0), (r1,c1))`。
type Rect = ((u32, u32), (u32, u32));

/// 手写期望的合并矩形（0 起，行主序）：
/// 标题 `A1:B1` → (0,0)-(0,1)；同值合并 `A` 段 `A3:A4` → (2,0)-(3,0)；
/// `B` 段 `A5:A6` → (4,0)-(5,0)。`C` 只有一行 ⇒ 不并（不出矩形）。
fn expected_rects() -> Vec<Rect> {
    vec![((0, 0), (0, 1)), ((2, 0), (3, 0)), ((4, 0), (5, 0))]
}

#[tokio::test]
async fn json_definition_renders_snapshot_with_merge_anchors() {
    let addr = common::spawn().await;
    let (body, ct) = form();
    let resp = common::post_to(&addr, "/render", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());

    // ① 接线等价：HTTP 面 ≡ 库函数直调。
    assert_eq!(resp.body_text(), library());

    // ② 几何真值：锚点矩形（0 起）与手写期望全等。
    let sh = sheet(&resp.body_text());
    assert_eq!(
        merge_rects(&sh),
        vec![(0, 0, 0, 1), (2, 0, 3, 0), (4, 0, 5, 0)],
        "合并锚点应为 标题 + 同值合并 A/B 两段"
    );

    // ③ 取值：明细扩展成 5 行（A/A/B/B/C），合计聚合 = 10+20+30+40+50。
    assert_eq!(cell_v(&sh, 0, 0), serde_json::json!("区域销售明细"));
    assert_eq!(cell_v(&sh, 1, 0), serde_json::json!("区域"));
    assert_eq!(cell_v(&sh, 1, 1), serde_json::json!("数量"));
    for (row, region, qty) in [
        (2, "A", 10),
        (3, "A", 20),
        (4, "B", 30),
        (5, "B", 40),
        (6, "C", 50),
    ] {
        assert_eq!(
            cell_v(&sh, row, 0),
            serde_json::json!(region),
            "第 {row} 行区域"
        );
        assert_eq!(
            cell_v(&sh, row, 1),
            serde_json::json!(qty),
            "第 {row} 行数量"
        );
    }
    // 合计行：明细 5 行扩展后落在 0 起第 7 行。
    assert_eq!(cell_v(&sh, 7, 0), serde_json::json!("合计"));
    assert_eq!(cell_v(&sh, 7, 1), serde_json::json!(150));

    // 行高属性表达式落地（0 起第 0 行 = 模板第 1 行 = 30mm ⇒ 113.39px）。
    let h = sh["rowData"]["0"]["h"].as_f64().unwrap();
    assert!(
        (h - (30.0 * 96.0 / 25.4)).abs() < 0.01,
        "标题行高应约 113.39px，实得 {h}"
    );
}

/// 从 xlsx 字节读回：首个 sheet 的取值 + 合并区矩形（0 起）。
fn read_xlsx(bytes: &[u8]) -> (calamine::Range<Data>, Vec<Rect>) {
    let mut wb: Xlsx<_> = calamine::open_workbook_from_rs(std::io::Cursor::new(bytes))
        .expect("响应体应是可读回的 xlsx");
    let name = wb.sheet_names().first().expect("至少一个 sheet").clone();
    let range = wb.worksheet_range(&name).expect("sheet 可读");
    let merged = wb.merge_cells_by_sheet_name(&name).expect("合并区可读");
    let mut rects: Vec<Rect> = merged.iter().map(|d| (d.start, d.end)).collect();
    rects.sort();
    (range, rects)
}

/// 1 起坐标取值（空白 ⇒ `Empty`）。
fn at(range: &calamine::Range<Data>, r: u32, c: u32) -> Data {
    range
        .get_value((r - 1, c - 1))
        .cloned()
        .unwrap_or(Data::Empty)
}

#[tokio::test]
async fn json_definition_exports_xlsx_with_merges_readback() {
    let addr = common::spawn().await;
    let (body, ct) = form();
    let resp = common::post_to(&addr, "/export?fmt=xlsx", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());

    // 合法 xlsx 的硬证据：zip 容器魔数。
    assert_eq!(&resp.body[..4], b"PK\x03\x04", "xlsx 应是 zip 容器");

    // 读回：合并区与取值都与**手写期望**全等（不经生产侧辅助函数）。
    let (range, rects) = read_xlsx(&resp.body);
    assert_eq!(
        rects,
        expected_rects(),
        "xlsx 读回的合并区应含标题 + 同值合并两段"
    );

    assert_eq!(at(&range, 1, 1), Data::String("区域销售明细".into()));
    assert_eq!(at(&range, 2, 1), Data::String("区域".into()));
    assert_eq!(at(&range, 2, 2), Data::String("数量".into()));
    // 明细区：锚点行有值，同值合并的延续槽**读回为空**（值只落锚点）。
    assert_eq!(at(&range, 3, 1), Data::String("A".into()));
    assert_eq!(at(&range, 3, 2), Data::Float(10.0));
    assert_eq!(at(&range, 4, 1), Data::Empty, "同值合并延续槽应为空");
    assert_eq!(at(&range, 4, 2), Data::Float(20.0));
    assert_eq!(at(&range, 5, 1), Data::String("B".into()));
    assert_eq!(at(&range, 6, 1), Data::Empty, "同值合并延续槽应为空");
    assert_eq!(at(&range, 7, 1), Data::String("C".into()));
    // 合计行（0 起第 7 行 = 1 起第 8 行）。
    assert_eq!(at(&range, 8, 1), Data::String("合计".into()));
    assert_eq!(at(&range, 8, 2), Data::Float(150.0));
}

#[tokio::test]
async fn json_definition_export_csv_matches_library() {
    // 第二出口（CSV）也吃 JSON 定义：与库函数直调逐字节一致（同一条主链路）。
    let addr = common::spawn().await;
    let (body, ct) = form();
    let resp = common::post_to(&addr, "/export?fmt=csv", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    assert_eq!(resp.header("content-type"), Some("text/csv; charset=utf-8"));
    let text = resp.body_text();
    assert!(text.contains("区域销售明细"), "CSV 应含标题：{text}");
    assert!(text.contains("合计"), "CSV 应含合计行：{text}");
}

/// 样式定义 JSON（M7-3）：三格分别覆盖「全字段 / 单字段 / 不声明」。
///
/// 矩阵 2×2（`Builder` 内部加一 ⇒ 快照是 3 行 × 3 列，0 起坐标下
/// 第 0 行列头、第 0 行第 0 列是左上角空白格）。断言只认**模板格**（1,1)/(1,2)/(2,1)。
///
/// 用 `r##` 而非 `r#`：JSON 里的颜色字面量含 `"#`，会撞上 `r#"..."#` 的收尾定界符。
const DEF_STYLE: &str = r##"{
  "version": 1,
  "grid": { "rows": 2, "cols": 2 },
  "cells": [
    { "row": 1, "col": 1, "text": "标题",
      "font_name": "微软雅黑", "font_size": 16,
      "bold": true, "underline": true,
      "fore_color": "#FF0000", "back_color": "#FFFFFF" },
    { "row": 1, "col": 2, "text": "斜体", "italic": true },
    { "row": 2, "col": 1, "text": "默认" }
  ]
}"##;

/// 一格的内联样式段 `cellData[r][c].s`。
fn cell_s(sheet: &serde_json::Value, r: usize, c: usize) -> serde_json::Value {
    sheet["cellData"][r.to_string()][c.to_string()]["s"].clone()
}

#[tokio::test]
async fn style_fields_reach_the_snapshot_verbatim() {
    let addr = common::spawn().await;
    let (body, ct) = common::multipart(&[("report", Some("def_style.json"), DEF_STYLE.as_bytes())]);
    let resp = common::post_to(&addr, "/render", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    let sh = sheet(&resp.body_text());

    // ① 全字段：JSON 写进去的 6 个样式槽必须**原样**出现在快照里。
    let t = cell_s(&sh, 0, 0);
    assert_eq!(t["ff"], serde_json::json!("微软雅黑"), "字体名");
    assert_eq!(t["fs"], serde_json::json!(16), "字号");
    assert_eq!(t["bl"], serde_json::json!(1), "粗体应置位");
    assert_eq!(t["ul"], serde_json::json!({ "s": 1 }), "下划线");
    assert_eq!(t["cl"]["rgb"], serde_json::json!("#FF0000"), "前景色");
    assert_eq!(t["bg"]["rgb"], serde_json::json!("#FFFFFF"), "背景色");
    assert!(t.get("it").is_none(), "未声明 italic ⇒ 不该出现 `it` 键");

    // ② 单字段：只声明 `italic` 不得改动其余槽（`ff`/`fs` 回到契约默认）。
    let i = cell_s(&sh, 0, 1);
    assert_eq!(i["it"], serde_json::json!(1), "斜体应置位");
    assert_eq!(i["fs"], serde_json::json!(12), "未声明字号 ⇒ 默认 12");
    assert!(
        i.get("bl").is_none() && i.get("ul").is_none(),
        "未声明粗体/下划线"
    );
    assert_eq!(i["cl"]["rgb"], serde_json::json!("#000000"), "前景色默认黑");

    // ③ 不声明：走 `build::blank_cell` 的契约默认（黑色前景 / 白色背景）。
    let d = cell_s(&sh, 1, 0);
    assert!(d.get("bl").is_none() && d.get("it").is_none() && d.get("ul").is_none());
    assert_eq!(d["fs"], serde_json::json!(12));
    assert_eq!(d["cl"]["rgb"], serde_json::json!("#000000"));
    assert_eq!(d["bg"]["rgb"], serde_json::json!("#FFFFFF"));
}

#[tokio::test]
async fn bad_style_field_is_400_with_json_path() {
    let addr = common::spawn().await;
    for (def, want) in [
        (
            r#"{"version":1,"grid":{"rows":1,"cols":1},
                "cells":[{"row":1,"col":1,"text":"x","fore_color":"red"}]}"#,
            "$.cells[0].fore_color",
        ),
        (
            r#"{"version":1,"grid":{"rows":1,"cols":1},
                "cells":[{"row":1,"col":1,"text":"x","font_size":0}]}"#,
            "$.cells[0].font_size",
        ),
    ] {
        let (body, ct) = common::multipart(&[("report", Some("bad.json"), def.as_bytes())]);
        let resp = common::post_to(&addr, "/render", body, &ct).await;
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
}

#[tokio::test]
async fn bad_json_definition_is_400_with_json_path() {
    // 结构错（列号越界）⇒ 400，且消息带 JSON 路径与「报表定义不合法」前缀。
    let addr = common::spawn().await;
    let bad = br#"{"grid":{"rows":2,"cols":2},"cells":[{"row":1,"col":9,"text":"x"}]}"#;
    let (body, ct) = common::multipart(&[("report", Some("def.json"), bad)]);
    let resp = common::post_to(&addr, "/render", body, &ct).await;
    assert_eq!(resp.status, 400, "响应体：{}", resp.body_text());
    let text = resp.body_text();
    assert!(text.contains("报表定义不合法"), "实得：{text}");
    assert!(
        text.contains("$.cells[0].col"),
        "应带 JSON 路径，实得：{text}"
    );
}

#[tokio::test]
async fn unknown_format_bytes_is_400() {
    // 既无 `RQQR` 魔数、也不是 JSON ⇒ 显式报「无法识别」（不猜、不静默当 .rpx 解）。
    let addr = common::spawn().await;
    let (body, ct) = common::multipart(&[("report", Some("weird.bin"), b"MZ\x90\x00garbage")]);
    let resp = common::post_to(&addr, "/render", body, &ct).await;
    assert_eq!(resp.status, 400, "响应体：{}", resp.body_text());
    assert!(
        resp.body_text().contains("无法识别的报表格式"),
        "实得：{}",
        resp.body_text()
    );
}

#[tokio::test]
async fn json_definition_uploads_as_rpx_asset() {
    // 资产库的唯一事实源是 `.rpx`（`reports.rpx BLOB`）：JSON 定义在入库前**归一化**为
    // `.rpx` ⇒ 上传即成功，且 `meta` 路由（读库即答，走 .rpx 解析）能正常应答。
    // 归一化之前这里是 400「模板解析失败」（`AssetStore::put` 的魔数预检）。
    let addr = common::spawn().await;
    let (body, ct) = common::multipart(&[("file", Some("def.json"), DEF.as_bytes())]);
    let resp = common::post_to(&addr, "/reports", body, &ct).await;
    assert_eq!(resp.status, 200, "响应体：{}", resp.body_text());
    let j: serde_json::Value = serde_json::from_str(&resp.body_text()).unwrap();
    assert_eq!(j["name"], "def", "报表名 = 文件名去扩展名");
    let id = j["id"].as_i64().expect("id 应为整数");

    // meta：`DEF` 本身不声明 `params` ⇒ 空清单；路由 200 本身证明库里的字节是
    // 合法 `.rpx`。（声明参数的读回见 `params_http.rs`。）
    let meta = common::get(&addr, &format!("/reports/{id}/meta")).await;
    assert_eq!(meta.status, 200, "响应体：{}", meta.body_text());
    assert_eq!(meta.body_text(), "[]", "v1 定义不声明参数");

    // 列表可见（归一化不得把条目丢掉）。
    let list = common::get(&addr, "/reports").await;
    assert!(
        list.body_text().contains("\"def\""),
        "实得：{}",
        list.body_text()
    );
}

#[tokio::test]
async fn bad_json_definition_is_rejected_by_upload_too() {
    // 归一化不得把非法定义放行入库（与 `/render` 同一套错误文本）。
    let addr = common::spawn().await;
    let bad = br#"{"grid":{"rows":0,"cols":1},"cells":[]}"#;
    let (body, ct) = common::multipart(&[("file", Some("bad.json"), bad)]);
    let resp = common::post_to(&addr, "/reports", body, &ct).await;
    assert_eq!(resp.status, 400, "响应体：{}", resp.body_text());
    assert!(
        resp.body_text().contains("报表定义不合法"),
        "实得：{}",
        resp.body_text()
    );
    // 拒收即不落库。
    let list = common::get(&addr, "/reports").await;
    assert_eq!(
        list.body_text(),
        "[]",
        "非法定义不得入库：{}",
        list.body_text()
    );
}
