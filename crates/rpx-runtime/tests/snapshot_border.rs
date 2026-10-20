//! 钉子：**模板里声明的边框必须进运行快照**（否则结果页与打印预览都丢边框）。
//!
//! # 为什么是钉子
//!
//! `snapshot::style::style_json` 早先把边框列为后置（`spec-m4.md` §4.3），只输出
//! 字体/颜色/对齐/数字格式 9 个字段。结果是：设计视图（直接读定义 JSON）看得到边框，
//! **结果页和打印预览白花花一片** —— 用户报「要可以加边框，可以反映到预览中」。
//!
//! 判据走真实链路（示例库复制到临时目录 → 改定义 → 保存 → 跑引擎 → 读快照），
//! 因为「JSON 字段 → `Cell.lb_*` 槽 → 快照 `s.bd`」中间有两道映射，只测任一端都会漏。
use rpx_runtime::{files, library, run};
use serde_json::{json, Value as J};
use std::{fs, path::Path};

const SAMPLE: &str = "store-ranking";
const MARK: &str = "合计";

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == ".runs" || name == ".lock" {
            continue;
        }
        let target = to.join(&name);
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[tokio::test]
async fn template_borders_reach_the_snapshot() {
    let temp = std::env::temp_dir().join(format!("rpx-border-{}", files::unique()));
    let library_root = temp.join("library");
    fs::create_dir_all(&library_root).unwrap();
    fs::write(library_root.join(".library.json"), b"{\"version\":1}\n").unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/library");
    copy_tree(&source.join(SAMPLE), &library_root.join(SAMPLE));

    let detail = library::show(&library_root, SAMPLE).unwrap();
    let revision = detail["revision"].as_str().unwrap().to_string();
    let mut template = detail["template"].clone();
    let cells = template["cells"].as_array_mut().unwrap();
    let row = cells
        .iter()
        .find(|cell| cell["text"] == MARK)
        .expect("示例报表应有合计行")["row"]
        .as_i64()
        .unwrap();
    // 上实线（1pt 红）+ 下双线（绿）+ 右虚线（2pt → MEDIUM_DASHED）。三种线型覆盖
    // 三个不同的 Univer 档位，映射错任何一条都会红。
    let target = cells
        .iter_mut()
        .find(|cell| cell["row"] == row && cell["col"] == 1)
        .expect("合计行第一格应存在");
    target["border"] = json!({
        "top": { "style": "solid", "width": 1.0, "color": "#FF0000" },
        "bottom": { "style": "double", "width": 1.0, "color": "#00FF00" },
        "right": { "style": "dashed", "width": 2.0, "color": "#0000FF" },
    });

    let saved = library::save(&library_root, SAMPLE, &revision, &template, false).unwrap();
    let started = library::execute(
        &library_root,
        SAMPLE,
        saved["revision"].as_str().unwrap(),
        &run::Options::default(),
    )
    .await
    .unwrap();
    let run_id = started["run"]["run_id"].as_str().unwrap();
    let path = library::artifact(&library_root, SAMPLE, run_id, "workbook.json").unwrap();
    let workbook: J = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let sheet = &workbook["sheets"][workbook["sheetOrder"][0].as_str().unwrap()];

    // 结果里的合计行（第一列就是模板的第 1 列，没参与横向扩展）
    let data = sheet["cellData"].as_object().unwrap();
    let (index, line) = data
        .iter()
        .find(|(_, line)| line["0"]["v"] == MARK)
        .expect("结果里应有合计行");
    let bd = &line["0"]["s"]["bd"];
    let _ = fs::remove_dir_all(&temp);

    assert_eq!(
        bd["t"]["s"], 1,
        "上边框：1pt solid ⇒ THIN(1)（快照第 {index} 行）"
    );
    assert_eq!(bd["t"]["cl"]["rgb"], "#FF0000", "上边框颜色");
    assert_eq!(bd["b"]["s"], 7, "下边框：double ⇒ DOUBLE(7)");
    assert_eq!(bd["b"]["cl"]["rgb"], "#00FF00", "下边框颜色");
    assert_eq!(bd["r"]["s"], 9, "右边框：2pt dashed ⇒ MEDIUM_DASHED(9)");
    // 没声明的那一边不输出（不是输出 `{s:0}`）—— 否则预览会把它当成显式「无边框」，
    // 连带把 `printOptions.borderWidth` 的网格线也挡掉。
    assert!(bd.get("l").is_none(), "未声明的左边框不应出现");
}
