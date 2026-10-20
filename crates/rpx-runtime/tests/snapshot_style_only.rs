//! 钉子：**同色整行**在结果快照里不能出现「中间断层」。
//!
//! # 为什么是钉子
//!
//! 快照 `cellData` 是稀疏的（只收 `CalcCell::is_emitted` 为真的格）。早先这条规则把
//! 「整行填色时那些**没有值**的格」也滤掉了 —— `store-ranking` 的合计行七格同色，
//! 结果页只剩四格有底色，中间出现白缝（用户报「合计这一行中间有断层」）。
//!
//! 判据：把合计行七格都设成同一底纹，跑完后该行**每一列都要有快照格**，且底色一致。
//! 用示例库的真实报表（复制到临时目录）而不是自造模板：这条链上的分页、扩展、
//! 合并都要一起参与才算数。
use rpx_runtime::{files, library, run};
use serde_json::{json, Value as J};
use std::{fs, path::Path};

const SAMPLE: &str = "store-ranking";
/// 合计行的标记文本（该行第一格）。
const MARK: &str = "合计";
const BAND: &str = "#EC1313";

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        // 运行历史不复制：几十 MB，且与判据无关。
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
async fn same_color_row_survives_without_gaps() {
    let temp = std::env::temp_dir().join(format!("rpx-band-{}", files::unique()));
    let library_root = temp.join("library");
    fs::create_dir_all(&library_root).unwrap();
    fs::write(library_root.join(".library.json"), b"{\"version\":1}\n").unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/library");
    copy_tree(&source.join(SAMPLE), &library_root.join(SAMPLE));

    let detail = library::show(&library_root, SAMPLE).unwrap();
    let revision = detail["revision"].as_str().unwrap().to_string();
    let mut template = detail["template"].clone();
    let cols = template["grid"]["cols"].as_i64().unwrap();
    let cells = template["cells"].as_array_mut().unwrap();

    // 找到合计行，并给该行**每一列**都上同一底纹（没有内容的列就新建一条纯样式格）
    let row = cells
        .iter()
        .find(|cell| cell["text"] == MARK)
        .expect("示例报表应有合计行")["row"]
        .as_i64()
        .unwrap();
    for col in 1..=cols {
        match cells
            .iter_mut()
            .find(|cell| cell["row"] == row && cell["col"] == col)
        {
            Some(cell) => {
                cell["back_color"] = json!(BAND);
            }
            None => cells.push(json!({"row": row, "col": col, "back_color": BAND})),
        }
    }

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

    // 结果里的合计行（行 0 起）应每列都有格，且底色都是同一色
    let data = sheet["cellData"].as_object().unwrap();
    let (index, result_row) = data
        .iter()
        .find(|(_, line)| line["0"]["v"] == MARK)
        .expect("结果里应有合计行");
    let colours: Vec<String> = (0..cols)
        .map(|col| {
            result_row[col.to_string()]["s"]["bg"]["rgb"]
                .as_str()
                .unwrap_or("<缺失>")
                .to_string()
        })
        .collect();
    let _ = fs::remove_dir_all(&temp);
    assert_eq!(
        colours,
        vec![BAND.to_string(); cols as usize],
        "合计行（快照第 {index} 行）必须整行同色，不能有断层"
    );
}
