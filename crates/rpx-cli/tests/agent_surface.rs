//! Agent 面契约：能力自描述、lint 闸门、数据探针、run 的版面摘要、错误提示。
//!
//! 这些测试只断言行面上的**可证伪事实**（退出码、错误码、hint 关键字），
//! 不复制实现细节——目的是让"文档说支持、二进制不支持"这类漂移当场失败。
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Temp(PathBuf);

impl Temp {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("rpx-agent-{tag}-{}", rpx_runtime::files::unique()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, body: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, body).unwrap();
        path
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn invoke(args: &[&str]) -> (i32, Value) {
    let Output { status, stdout, .. } = Command::new(env!("CARGO_BIN_EXE_rpx"))
        .args(args)
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&stdout)
        .unwrap_or_else(|_| panic!("stdout 不是 JSON：{}", String::from_utf8_lossy(&stdout)));
    (status.code().unwrap_or(-1), value)
}

fn csv() -> &'static str {
    "dept,item,amount\n销售部,差旅,12000\n销售部,招待,8000\n研发部,设备,30000\n"
}

fn template(delimiter: bool) -> Value {
    let mut file = json!({"file_name": "expense.csv", "encoding": "UTF-8"});
    if delimiter {
        file["delimiter"] = json!(",");
    }
    json!({
        "version": 1,
        "grid": {"rows": 5, "cols": 3},
        "datasets": [{"name": "exp", "file": file}],
        "col_props": [
            {"col": 1, "width": 30}, {"col": 2, "width": 30}, {"col": 3, "width": 26}
        ],
        "row_props": [
            {"row": 1, "height": 10}, {"row": 3, "height": 8},
            {"row": 4, "height": 8}, {"row": 5, "height": 8}
        ],
        "printing": {"auto_size": false},
        "cells": [
            {"row": 1, "col": 1, "text": "部门费用明细表", "font_size": 16, "bold": true,
             "merge": {"rows": 1, "cols": 3}},
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

fn fixture(tag: &str) -> (Temp, PathBuf) {
    let temp = Temp::new(tag);
    temp.write("expense.csv", csv());
    let path = temp.write("template.json", &template(true).to_string());
    (temp, path)
}

#[test]
fn capabilities_is_self_describing() {
    let (code, value) = invoke(&["capabilities"]);
    assert_eq!(code, 0, "{value}");
    let data = &value["data"];
    assert_eq!(data["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(data["features"]["lint"], true);
    assert_eq!(data["features"]["probe_data"], true);

    let commands: Vec<&str> = data["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();
    for required in ["lint", "capabilities", "run", "template.validate"] {
        assert!(
            commands.contains(&required),
            "缺命令 {required}：{commands:?}"
        );
    }

    let exports: Vec<&str> = data["exports"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for required in ["csv", "xlsx", "print-xlsx", "html", "pdf"] {
        assert!(
            exports.contains(&required),
            "缺导出格式 {required}：{exports:?}"
        );
    }

    let errors = data["errors"].as_array().unwrap();
    let calculation = errors
        .iter()
        .find(|e| e["code"] == "CALCULATION_FAILED")
        .expect("错误目录应含 CALCULATION_FAILED");
    assert_eq!(calculation["hint"], true, "该错误码必须带 hint");
    assert_eq!(calculation["exit"], 5);
}

#[test]
fn lint_reports_silent_failures_and_gates_strict_mode() {
    let temp = Temp::new("lint");
    temp.write("expense.csv", csv());
    let mut broken = template(true);
    broken["cells"][0]["text"] = json!("${period} 部门费用明细表");
    let path = temp.write("broken.json", &broken.to_string());
    let path = path.to_str().unwrap();

    let (code, value) = invoke(&["lint", path]);
    assert_eq!(code, 0, "默认模式只有告警，不该失败：{value}");
    assert_eq!(value["data"]["summary"]["codes"], json!(["L001"]));
    assert_eq!(value["data"]["gate"]["passed"], true);
    assert!(value["data"]["issues"][0]["hint"].is_string());

    let (code, value) = invoke(&["lint", path, "--strict"]);
    assert_eq!(code, 3, "strict 下告警要让 CI 判失败：{value}");
    assert_eq!(value["data"]["gate"]["exit_code"], 3);
}

#[test]
fn validate_probe_data_exposes_unsplit_csv() {
    let temp = Temp::new("probe");
    temp.write("expense.csv", csv());
    let path = temp.write("template.json", &template(false).to_string());
    let path = path.to_str().unwrap();

    // 静态校验照旧通过——这正是问题：validate 不读数据。
    let (code, value) = invoke(&["template", "validate", path]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["data"]["probe"], Value::Null);

    // 探针才把它揪出来，且指到 delimiter 字段。
    let (code, value) = invoke(&["template", "validate", path, "--probe-data"]);
    assert_eq!(code, 4, "{value}");
    assert_eq!(value["error"]["code"], "DATA_SHAPE");
    assert_eq!(value["error"]["json_pointer"], "/datasets/0/file/delimiter");
    assert!(value["error"]["hint"]
        .as_str()
        .unwrap()
        .contains("\"delimiter\": \",\""));
}

#[test]
fn probe_warns_on_unknown_column_without_failing() {
    let temp = Temp::new("probe-col");
    temp.write("expense.csv", csv());
    let mut typo = template(true);
    typo["cells"][6]["expr"] = json!("exp.sum(amountt,dept==A4 and item==B4)");
    let path = temp.write("typo.json", &typo.to_string());

    let (code, value) = invoke(&[
        "template",
        "validate",
        path.to_str().unwrap(),
        "--probe-data",
    ]);
    assert_eq!(code, 0, "列名存疑只是提示，不该阻断：{value}");
    let warnings = value["data"]["probe"]["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{value}");
    assert_eq!(warnings[0]["code"], "DATA_COLUMN");
    assert!(warnings[0]["message"].as_str().unwrap().contains("amountt"));
    assert!(warnings[0]["message"].as_str().unwrap().contains("amount"));
}

#[test]
fn run_returns_layout_and_actionable_hint() {
    let (_temp, path) = fixture("run");
    let out =
        Path::new(&std::env::temp_dir()).join(format!("rpx-run-{}", rpx_runtime::files::unique()));
    let (code, value) = invoke(&[
        "run",
        path.to_str().unwrap(),
        "--out-dir",
        out.to_str().unwrap(),
        "--export",
        "csv",
    ]);
    assert_eq!(code, 0, "{value}");
    let layout = &value["data"]["layout"];
    assert_eq!(layout["paginated"], true, "{layout}");
    assert!(layout["page_count"].as_i64().unwrap() >= 1);
    assert!(layout["rows"].as_i64().unwrap() >= 4);
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn unsplit_csv_error_carries_delimiter_hint() {
    let temp = Temp::new("hint");
    temp.write("expense.csv", csv());
    let path = temp.write("template.json", &template(false).to_string());
    let out = temp.0.join("out");
    let (code, value) = invoke(&[
        "run",
        path.to_str().unwrap(),
        "--out-dir",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, 5, "缺 delimiter 在计算阶段炸：{value}");
    assert_eq!(value["error"]["code"], "CALCULATION_FAILED");
    let hint = value["error"]["hint"].as_str().unwrap_or_default();
    assert!(hint.contains("delimiter"), "hint 必须指向修法：{hint}");
    assert!(
        hint.contains("customer") || hint.contains("dept,item,amount"),
        "{hint}"
    );
}

/// 页数三处必须一致：分页器的 `layout.page_count`、HTML 的 `<section>` 数、
/// PDF 的页对象数。**只对齐一处等于没对齐**——PDF 里多一页空白最难被发现。
#[test]
fn run_exports_paged_html_and_optional_pdf() {
    let (_temp, path) = fixture("page");
    let out =
        Path::new(&std::env::temp_dir()).join(format!("rpx-page-{}", rpx_runtime::files::unique()));
    let chrome = rpx_runtime::pdf::chrome();
    let mut args = vec![
        "run",
        path.to_str().unwrap(),
        "--out-dir",
        out.to_str().unwrap(),
        "--export",
        "html",
    ];
    if chrome.is_some() {
        args.extend(["--export", "pdf"]);
    }
    let (code, value) = invoke(&args);
    assert_eq!(code, 0, "{value}");
    let pages = value["data"]["layout"]["page_count"].as_u64().unwrap();

    let html = fs::read_to_string(out.join("artifacts/report.html")).unwrap();
    assert_eq!(
        html.matches("<section class=\"page\"").count() as u64,
        pages,
        "HTML 页数与分页器不一致"
    );
    assert!(html.contains("@page{size:"), "缺 @page 纸张声明");

    if chrome.is_some() {
        let pdf = fs::read(out.join("artifacts/report.pdf")).unwrap();
        assert!(pdf.starts_with(b"%PDF"), "PDF 文件头不对");
        let total = count(&pdf, b"/Type /Page");
        let tree = count(&pdf, b"/Type /Pages");
        assert_eq!((total - tree) as u64, pages, "PDF 页数与分页器不一致");
    }
    let _ = fs::remove_dir_all(&out);
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}
