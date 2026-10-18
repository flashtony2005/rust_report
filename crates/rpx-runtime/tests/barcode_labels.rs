//! 条码标签的错位排版与 `barcode()` 的端到端判据。
//!
//! 这个例子有三件事可能悄悄坏掉，而且**都不会报错**：
//! ① 条码没编进数据（每行同一张图）—— 看着有图，内容全一样；
//! ② 条件出图失效（两边都出图 / 两边都不出）—— 版面塌成单列或全空；
//! ③ 产出的 PNG 浏览器能显示但导出器解不开 —— 预览正常、Excel 里没图。
//!
//! 所以判据不看「有没有图」，而是逐行看**哪一侧**是真条码、**图与图是否不同**、
//! 以及每张图能否过导出侧的真解析器。
use rpx_engine::params::ParamTable;
use rpx_runtime::{execution::render_request, form::RenderRequest};
use serde_json::Value as J;
use std::{collections::BTreeMap, path::Path};

/// 条件出图「关」的那一侧用的全透明 1×1 PNG（与模板里同一个常量）。
const BLANK: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNgAAIAAAUAAen63NgAAAAASUVORK5CYII=";

fn render() -> J {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/r-model");
    let req = RenderRequest {
        report: std::fs::read(dir.join("barcode-staggered-labels.json")).unwrap(),
        report_name: "barcode-staggered-labels".into(),
        params: ParamTable::EMPTY,
        datasets: BTreeMap::from([(
            "barcode-labels.csv".into(),
            std::fs::read(dir.join("barcode-labels.csv")).unwrap(),
        )]),
    };
    serde_json::from_str(&render_request(&req).unwrap()).unwrap()
}

/// 每一行的两个图形槽：`(列, 是否真条码)`，按列升序。
fn rows(book: &J) -> Vec<Vec<(i64, bool)>> {
    let graphics = book["sheets"]["rpx-sheet-1"]["rpxGraphics"]
        .as_array()
        .expect("workbook 必须有 rpxGraphics");
    let mut by_row: BTreeMap<i64, Vec<(i64, bool)>> = BTreeMap::new();
    for g in graphics {
        let source = g["content"]["source"].as_str().unwrap_or_default();
        by_row
            .entry(g["row"].as_i64().unwrap())
            .or_default()
            .push((g["col"].as_i64().unwrap(), !source.contains(BLANK)));
    }
    for slots in by_row.values_mut() {
        slots.sort_unstable();
    }
    by_row.into_values().collect()
}

#[test]
fn every_label_gets_exactly_one_barcode_alternating_sides() {
    let book = render();
    let rows = rows(&book);
    assert_eq!(rows.len(), 10, "10 条标签数据应产生 10 行");
    for (i, slots) in rows.iter().enumerate() {
        assert_eq!(slots.len(), 2, "第 {} 行应有两个图形槽（左右各一）", i + 1);
        let real: Vec<i64> = slots
            .iter()
            .filter(|(_, real)| *real)
            .map(|(c, _)| *c)
            .collect();
        assert_eq!(
            real.len(),
            1,
            "第 {} 行必须只有一张真条码：{slots:?}",
            i + 1
        );
        // 奇数号（行下标偶数）条码在左，偶数号在右 —— 这就是「错位」。
        let want = if i % 2 == 0 { 1 } else { 4 };
        assert_eq!(real[0], want, "第 {} 行的条码应在第 {want} 列", i + 1);
    }
}

#[test]
fn barcode_content_follows_the_row_data() {
    let book = render();
    let sources: Vec<String> = book["sheets"]["rpx-sheet-1"]["rpxGraphics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|g| g["content"]["source"].as_str())
        .filter(|s| !s.contains(BLANK))
        .map(str::to_string)
        .collect();
    assert_eq!(sources.len(), 10, "应有 10 张真条码");
    // 每张图必须互不相同 —— 否则说明 barcode() 没吃到当前行的 code，
    // 十张标签印同一个条码（看着完全正常，扫出来全一样）。
    let mut unique = sources.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), sources.len(), "十张条码的内容必须各不相同");
    // 且必须是 PNG data URI，并且能被导出侧的真解析器解开。
    for source in &sources {
        assert!(
            source.starts_with("data:image/png;base64,"),
            "条码必须是 PNG"
        );
        rpx_export::validate_image(source).expect("条码 PNG 必须能被导出器解析");
    }
}

#[test]
fn non_ascii_barcode_content_is_rejected() {
    // Code 128B 编不了中文：必须报错，不能静默出一张空白图。
    // 自带最小定义，不依赖示例模板的写法 —— 否则改模板会把这条判据变成假绿/假红。
    let definition = serde_json::json!({
        "grid": {"rows": 2, "cols": 2},
        "datasets": [{"name":"lbl","file":{"file_name":"barcode-labels.csv","encoding":"UTF-8","delimiter":","}}],
        "cells": [
            {"row":1,"col":1,"extend_mode":"vertical","expr":"lbl.select(code)"},
            {"row":1,"col":2,"data_type":"image","left_head":"A1",
             "image":{"source":"=barcode('华东')","width":100,"height":30}}
        ]
    });
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/r-model");
    let req = RenderRequest {
        report: serde_json::to_vec(&definition).unwrap(),
        report_name: "barcode".into(),
        params: ParamTable::EMPTY,
        datasets: BTreeMap::from([(
            "barcode-labels.csv".into(),
            std::fs::read(dir.join("barcode-labels.csv")).unwrap(),
        )]),
    };
    let error = format!("{:?}", render_request(&req).unwrap_err());
    assert!(
        error.contains("Code 128B"),
        "应报 Code 128B 编码错误：{error}"
    );
}
