//! `lint` 的靶子：四类**静默失败**（validate 通过、产物却是错的）。
//!
//! 每个用例都是最小对照：同一个可用基线，只改一处。断言的是"改哪一处，
//! 就必须报哪一条"，而不是"大概有几条告警"——检查项漂移时测试要能失败。
use rpx_runtime::lint::lint;
use serde_json::{json, Value as J};

/// 一份干净的两级分组报表：部门 → 费用项 → 金额 + 合计。
fn baseline() -> J {
    json!({
        "version": 1,
        "grid": {"rows": 6, "cols": 4},
        "datasets": [{"name": "exp", "file": {
            "file_name": "expense.csv", "encoding": "UTF-8", "delimiter": ","
        }}],
        "col_props": [
            {"col": 1, "width": 30}, {"col": 2, "width": 30},
            {"col": 3, "width": 26}, {"col": 4, "width": 20}
        ],
        "row_props": [
            {"row": 1, "height": 12}, {"row": 3, "height": 8},
            {"row": 4, "height": 8}, {"row": 5, "height": 8}
        ],
        "printing": {"auto_size": false},
        "cells": [
            {"row": 1, "col": 1, "text": "部门费用明细表", "font_size": 16, "bold": true,
             "h_align": "center", "merge": {"rows": 1, "cols": 4}},
            {"row": 3, "col": 1, "text": "部门", "bold": true},
            {"row": 3, "col": 2, "text": "费用项", "bold": true},
            {"row": 3, "col": 3, "text": "金额", "bold": true},
            {"row": 4, "col": 1, "extend_mode": "vertical", "expr": "exp.group(dept)"},
            {"row": 4, "col": 2, "extend_mode": "none", "left_head": "A4",
             "expr": "exp.select(item,dept==A4)"},
            {"row": 4, "col": 3, "extend_mode": "none", "left_head": "A4",
             "expr": "exp.sum(amount,dept==A4 and item==B4)", "format": "#,##0"},
            {"row": 5, "col": 1, "text": "合计", "bold": true},
            {"row": 5, "col": 3, "expr": "exp.sum(amount)", "format": "#,##0", "bold": true}
        ]
    })
}

fn codes(template: &J) -> Vec<String> {
    let report = lint(template, None);
    report["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|i| i["code"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn clean_template_reports_nothing() {
    let report = lint(&baseline(), None);
    assert_eq!(report["summary"]["total"], 0, "{report}");
    assert_eq!(report["gate"], J::Null, "gate 由 CLI 加，runtime 不该自造");
}

#[test]
fn macro_in_text_is_flagged() {
    let mut template = baseline();
    template["cells"][0]["text"] = json!("${period} 部门费用明细表");
    let report = lint(&template, None);
    let issue = &report["issues"][0];
    assert_eq!(issue["code"], "L001");
    assert_eq!(issue["level"], "warning");
    assert_eq!(issue["json_pointer"], "/cells/0/text");
    assert!(issue["hint"].as_str().unwrap().contains("expr"), "{issue}");
}

#[test]
fn declared_border_is_flagged_once() {
    let mut template = baseline();
    let border = json!({"top": {"style": "solid", "color": "#000000"}});
    for index in [1, 2, 3] {
        template["cells"][index]["border"] = border.clone();
    }
    let report = lint(&template, None);
    let flagged: Vec<&J> = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["code"] == "L004")
        .collect();
    assert_eq!(flagged.len(), 1, "同类问题合成一条，避免刷屏：{report}");
    assert!(flagged[0]["message"]
        .as_str()
        .unwrap()
        .contains("3 个单元格"));
}

#[test]
fn blank_row_inside_extension_block_is_flagged() {
    let mut template = baseline();
    // 把合计行从 5 推到 6，第 5 行留空：扩展块 4 → 空 5 → 合计 6。
    template["cells"][7]["row"] = json!(6);
    template["cells"][8]["row"] = json!(6);
    template["grid"]["rows"] = json!(7);
    let report = lint(&template, None);
    assert_eq!(codes(&template), vec!["L003".to_string()], "{report}");
    assert!(report["issues"][0]["message"]
        .as_str()
        .unwrap()
        .contains("第 5 行"));
}

#[test]
fn parallel_vertical_roots_are_flagged_but_sibling_chain_is_not() {
    // 病态：两个 vertical 各自为根 ⇒ 扩展相乘。
    let mut parallel = baseline();
    parallel["cells"][5]["extend_mode"] = json!("vertical");
    parallel["cells"][5]["expr"] = json!("exp.group(item)");
    parallel["cells"][5]
        .as_object_mut()
        .unwrap()
        .remove("left_head");
    assert_eq!(codes(&parallel), vec!["L002".to_string()]);

    // 合法：B4 的 left_head 指向 A4，收敛到唯一根。
    let mut chained = baseline();
    chained["cells"][5]["extend_mode"] = json!("vertical");
    chained["cells"][5]["expr"] = json!("exp.group(item)");
    chained["cells"][5]["left_head"] = json!("A4");
    assert!(codes(&chained).is_empty(), "{}", lint(&chained, None));

    // 合法：兄弟共享父格（detail-merge-same 的写法），也不能报。
    let mut siblings = baseline();
    siblings["cells"][5]["extend_mode"] = json!("vertical");
    siblings["cells"][5]["expr"] = json!("exp.group(item)");
    siblings["cells"][5]["left_head"] = json!("A4");
    siblings["cells"][6]["left_head"] = json!("B4");
    assert!(codes(&siblings).is_empty(), "{}", lint(&siblings, None));
}

#[test]
fn missing_delimiter_and_missing_file_are_flagged() {
    let mut template = baseline();
    template["datasets"][0]["file"]
        .as_object_mut()
        .unwrap()
        .remove("delimiter");
    assert!(codes(&template).contains(&"L006".to_string()));

    let directory = std::env::temp_dir().join(format!("rpx-lint-{}", rpx_runtime::files::unique()));
    std::fs::create_dir_all(&directory).unwrap();
    let report = lint(&baseline(), Some(&directory));
    let found = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["code"] == "L005");
    assert!(found, "文件不存在应报 L005：{report}");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn wide_columns_and_narrow_decimals_are_flagged() {
    let mut wide = baseline();
    for (index, width) in [60, 60, 60, 60].into_iter().enumerate() {
        wide["col_props"][index]["width"] = json!(width);
    }
    assert!(codes(&wide).contains(&"L007".to_string()));

    let mut narrow = baseline();
    narrow["col_props"][2]["width"] = json!(12);
    narrow["cells"][6]["format"] = json!("#,##0.00");
    let report = lint(&narrow, None);
    assert!(codes(&narrow).contains(&"L008".to_string()), "{report}");
}

#[test]
fn canvas_templates_are_exempt_from_page_width() {
    let mut canvas = baseline();
    canvas["cells"][2]["chart"] = json!({"type": "bar"});
    for (index, width) in [80, 80, 80, 80].into_iter().enumerate() {
        canvas["col_props"][index]["width"] = json!(width);
    }
    assert!(!codes(&canvas).contains(&"L007".to_string()));
}
