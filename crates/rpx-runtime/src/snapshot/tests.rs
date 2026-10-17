//! 快照映射单测（纯函数）+ 夹具端到端（`engine_merge` 真实数据，不造假）。

use std::fs::File;
use std::path::PathBuf;

use rpx_engine::value::Value;

use super::{geom, style, value};

// ---------- 值映射（`docs/spec-m4.md` §4.3.1） ----------

#[test]
fn text_is_force_string_so_numeric_text_stays_text() {
    let (v, t) = value::value_json(&Value::Text("00123".to_string()));
    assert_eq!(v, serde_json::json!("00123"));
    assert_eq!(t, Some(value::T_FORCE_STRING));
}

#[test]
fn int_and_decimal_map_to_number() {
    let (v, t) = value::value_json(&Value::Int(7));
    assert_eq!(v, serde_json::json!(7));
    assert_eq!(t, Some(2));
    // Decimal → number 是声明近似（尾零丢失由 `n` 后置补，§7）；此处只钉"是数字不是字符串"
    let (v, t) = value::value_json(&Value::Decimal("100.00".to_string()));
    assert_eq!(v, serde_json::json!(100.0));
    assert_eq!(t, Some(2));
}

#[test]
fn nan_maps_to_null_not_zero() {
    let (v, t) = value::value_json(&Value::Double(f64::NAN));
    assert_eq!(v, serde_json::Value::Null);
    assert_eq!(t, None);
}

#[test]
fn list_joins_with_commas() {
    let (v, _) = value::value_json(&Value::List(vec![
        Value::Int(1),
        Value::Text("a".to_string()),
    ]));
    assert_eq!(v, serde_json::json!("1,a"));
}

// ---------- 颜色与对齐（`spec-m4.md` §4.3） ----------

#[test]
fn argb_keeps_low_24_bits() {
    assert_eq!(style::argb_hex(0xFF00_0000u32 as i32), "#000000");
    assert_eq!(style::argb_hex(-1), "#FFFFFF");
    assert_eq!(style::argb_hex(0x00FF_0000), "#FF0000");
}

#[test]
fn align_codes_follow_contract_note3() {
    // h：-48 left / -47 center / -46 right / -45 justified / -44 distributed
    assert_eq!(style::h_align_code(-48), Some(1));
    assert_eq!(style::h_align_code(-47), Some(2));
    assert_eq!(style::h_align_code(-46), Some(3));
    assert_eq!(style::h_align_code(-45), Some(4));
    assert_eq!(style::h_align_code(-44), Some(6));
    assert_eq!(style::h_align_code(0), None, "未知值不猜");
    // v：-32 top / -31 middle / -30 bottom
    assert_eq!(style::v_align_code(-32), Some(1));
    assert_eq!(style::v_align_code(-31), Some(2));
    assert_eq!(style::v_align_code(-30), Some(3));
    assert_eq!(style::v_align_code(0), None);
}

// ---------- 几何（`spec-m4.md` §4.2） ----------

#[test]
fn defaults_convert_mm_to_px() {
    // 8mm ≈ 30px、25mm ≈ 94px（96dpi 近似，声明见 §7）
    assert_eq!(geom::default_row_height_px(), 30);
    assert_eq!(geom::default_col_width_px(), 94);
}

// ---------- 端到端：夹具（真实数据，不造假） ----------

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// 跑一个夹具：`.rpx` → 数据集 → 计算 → 快照 → 解析回 JSON。
fn wb_of(fixture: &str) -> serde_json::Value {
    let dir = fixture_dir();
    let file = File::open(dir.join(format!("{fixture}.rpx"))).expect("夹具存在");
    let rd = rpx_core::open(file).expect("解析 .rpx");
    let ds = rpx_engine::dataset::load_datasets(&dir, &rd).expect("载入数据集");
    let grid = rpx_engine::calculate(&rd, &ds).expect("计算");
    let json = super::build(&rd, &grid, fixture).expect("快照生成（含 merge 网格校验）");
    serde_json::from_str(&json).expect("输出必须是合法 JSON")
}

#[test]
fn engine_merge_snapshot_has_six_merge_anchors_at_zero_based_coords() {
    let wb = wb_of("engine_merge");
    let sheet = &wb["sheets"]["rpx-sheet-1"];

    let merges = sheet["mergeData"].as_array().expect("mergeData 是数组");
    // 实证：`engine_merge.expected.json` 的 `merged.cells` 12 槽 = 6 块 × 2 槽（§4.4）
    assert_eq!(merges.len(), 6, "6 个合并锚点，实际: {merges:?}");
    // 逐块核对（引擎 1 起 → Univer 0 起）：
    //   North (2,1)纵2 / span (2,3)横2 / East (4,1)纵2 / span (4,3)横2 / South (6,1)纵2 / span (6,3)横2
    let want = [
        (1, 0, 2, 0),
        (1, 2, 1, 3),
        (3, 0, 4, 0),
        (3, 2, 3, 3),
        (5, 0, 6, 0),
        (5, 2, 5, 3),
    ];
    for (i, (sr, sc, er, ec)) in want.iter().enumerate() {
        assert_eq!(merges[i]["startRow"], *sr, "第 {i} 块 startRow：{merges:?}");
        assert_eq!(merges[i]["startColumn"], *sc, "第 {i} 块 startColumn");
        assert_eq!(merges[i]["endRow"], *er, "第 {i} 块 endRow");
        assert_eq!(merges[i]["endColumn"], *ec, "第 {i} 块 endColumn");
    }

    // cellData：非空、键为 0 起字符串
    let cd = sheet["cellData"].as_object().expect("cellData 是对象");
    assert!(!cd.is_empty(), "cellData 不能为空");
    assert!(
        cd.keys().all(|k| k.parse::<usize>().is_ok()),
        "行键是数字串: {cd:?}"
    );

    // rowData / columnData：非空、形如 {"h":N} / {"w":N}
    let rdata = sheet["rowData"].as_object().expect("rowData 是对象");
    assert!(!rdata.is_empty());
    assert!(rdata.values().next().unwrap()["h"].is_number());
    let cdata = sheet["columnData"].as_object().expect("columnData 是对象");
    assert!(!cdata.is_empty());
    assert!(cdata.values().next().unwrap()["w"].is_number());
}

#[test]
fn engine_samemerge_recovers_two_anchors_from_merged_slots() {
    // `engine_samemerge` 的跨度**只在 `merged` 段可见**（`source` 段读不到，
    // `calc/run.rs:428-459` 注释）—— 若实现误用 source 段，此断言必红。
    let wb = wb_of("engine_samemerge");
    let merges = wb["sheets"]["rpx-sheet-1"]["mergeData"]
        .as_array()
        .expect("mergeData 是数组");
    assert_eq!(merges.len(), 2, "4 槽 = 2 块 × 2 槽，实际: {merges:?}");
    assert_eq!(merges[0]["startRow"], 1);
    assert_eq!(merges[0]["endRow"], 2);
    assert_eq!(merges[1]["startRow"], 3);
    assert_eq!(merges[1]["endRow"], 4);
}

#[test]
fn merge_grid_validation_fails_loudly_on_violation() {
    // 负路径（诚实失败优于静默错）：人为造一个缺延续槽的跨度 ⇒ 必须报错。
    let mut g = rpx_engine::arena::CalcGrid::new(4, 2);
    g.merged.push(rpx_engine::arena::MergedCell::new(
        1,
        2,
        1,
        Value::Null,
        2,
        1,
    ));
    let err = geom::merge_data(&g).expect_err("缺槽必须报错（不静默少报一块）");
    assert!(err.contains("缺槽"), "错误需说明缺槽: {err}");
}

// ---- M4-05 多 sheet 翻页（paged 切片，`docs/spec-m4.md` §4.6）----

/// 真实夹具 `engine_pager`：真值口径 4 页（row_range 页首重叠——区带副本如实）。
/// 断言：4 sheet 顺序即页序、名「第 N 页」、每页 cellData 键 0 起且含区带副本、
/// merged 在多页夹具下为空（`MergedCell` 文档的已声明缺口）。
#[test]
fn paged_snapshot_engine_pager_has_four_sheets_with_translated_rows() {
    let wb = wb_of("engine_pager");
    assert_eq!(
        wb["sheetOrder"],
        serde_json::json!(["rpx-sheet-1", "rpx-sheet-2", "rpx-sheet-3", "rpx-sheet-4"])
    );
    assert_eq!(wb["sheets"]["rpx-sheet-1"]["name"], "第 1 页");
    assert_eq!(wb["sheets"]["rpx-sheet-4"]["name"], "第 4 页");
    // 顶层 pagination 观测块保留（§4.5）。
    assert_eq!(wb["pagination"]["page_count"], 4);
    // 每页 cellData 键从 0 起（切片平移）；页 2 含区带副本行 0（row_range [1,22] 0 起
    // ⇒ 页首重叠在页内表现为第 0 行就是区带行）。
    for sid in ["rpx-sheet-1", "rpx-sheet-2", "rpx-sheet-3", "rpx-sheet-4"] {
        let cd = &wb["sheets"][sid]["cellData"];
        assert!(cd.get("0").is_some(), "{sid} 页内应含 0 行（区带副本）");
        // engine_pager 计算网格本身无合并槽 ⇒ 各页 mergeData 为空（不是缺口，
        // 跨页拆分语义由 engine_merge 自造分页的专项测试覆盖）。
        assert_eq!(
            wb["sheets"][sid]["mergeData"].as_array().unwrap().len(),
            0,
            "{sid} 的 merged 应为空（网格无合并槽）"
        );
    }
    // 页 4 row_range（0 起 0..41）⇒ 页内行键**全部 ≤ 41**（末页不满页也不越界）。
    for k in wb["sheets"]["rpx-sheet-4"]["cellData"]
        .as_object()
        .unwrap()
        .keys()
    {
        assert!(k.parse::<u32>().unwrap() <= 41, "页 4 行键越页界：{k}");
    }
}

/// 跨页合并拆分（产品语义，`SimplePager.java:1624-1638` 行向 / `:2316-2335` 列向）：
/// 用真实夹具 `engine_merge` 的计算网格 + **自造分页**（页界切在 North 块
/// 0 起 (1,0)-(2,0) 的行 1/2 之间）驱动 `build_paged`。断言：
/// ① 上页块裁到页末；② 延续页有独立块 + **克隆锚点**（值 = 原锚点值）；
/// ③ 锚点所在页之外的其他块照常按交叠平移。
#[test]
fn paged_snapshot_splits_cross_page_merge_with_cloned_anchor() {
    let dir = fixture_dir();
    let file = File::open(dir.join("engine_merge.rpx")).expect("夹具存在");
    let rd = rpx_core::open(file).expect("解析 .rpx");
    let ds = rpx_engine::dataset::load_datasets(&dir, &rd).expect("载入数据集");
    let grid = rpx_engine::calculate(&rd, &ds).expect("计算");
    let total_rows = grid.row_heights.len() as i32;
    let total_cols = grid.col_widths.len() as i32;
    let pg = rpx_engine::pager::Pagination {
        pages: vec![
            rpx_engine::pager::PageInfo {
                panels: Vec::new(),
                page: 1,
                rows: 2,
                col1_values: vec![],
                row_indices: (1..=2).collect(),
                row_range: (1, 2),
                col_indices: (1..=total_cols).collect(),
                col_range: (1, total_cols),
            },
            rpx_engine::pager::PageInfo {
                panels: Vec::new(),
                page: 2,
                rows: total_rows - 2,
                col1_values: vec![],
                row_indices: (3..=total_rows).collect(),
                row_range: (3, total_rows),
                col_indices: (1..=total_cols).collect(),
                col_range: (1, total_cols),
            },
        ],
        ..Default::default()
    };
    let json = super::paged::build_paged(&rd, &grid, &pg, "engine_merge").expect("多页快照");
    let wb: serde_json::Value = serde_json::from_str(&json).unwrap();

    // 页 1（全局 0 起行 0..1）：North 块（0 起行 1..2）交叠裁到 (1,0)-(1,0)；
    // 横 2 块（0 起行 1）交叠 (1,2)-(1,3)；East/South 块（行 3..6）不在此页。
    let p1 = wb["sheets"]["rpx-sheet-1"]["mergeData"].clone();
    assert_eq!(
        p1,
        serde_json::json!([
            {"startRow": 1, "startColumn": 2, "endRow": 1, "endColumn": 3},
        ]),
        "页 1 合并块：{p1:?}"
    );

    // 页 2（全局 0 起行 2 起）：North 延续块 (0,0)-(0,0)；East/South 纵块与两个
    // 横向 span 块按交叠平移。
    let p2 = wb["sheets"]["rpx-sheet-2"]["mergeData"].clone();
    assert_eq!(
        p2,
        serde_json::json!([
            {"startRow": 1, "startColumn": 0, "endRow": 2, "endColumn": 0},
            {"startRow": 1, "startColumn": 2, "endRow": 1, "endColumn": 3},
            {"startRow": 3, "startColumn": 0, "endRow": 4, "endColumn": 0},
            {"startRow": 3, "startColumn": 2, "endRow": 3, "endColumn": 3},
        ]),
        "页 2 合并块：{p2:?}"
    );

    // 克隆锚点：页 2 的 (0,0) 值 = 页 1 North 锚点 (1,0) 的值（deepClone 快照等价）。
    let anchor_v = &wb["sheets"]["rpx-sheet-1"]["cellData"]["1"]["0"]["v"];
    let clone_v = &wb["sheets"]["rpx-sheet-2"]["cellData"]["0"]["0"]["v"];
    assert_eq!(anchor_v, clone_v, "延续页克隆锚点值必须等于原锚点值");
    assert!(!clone_v.is_null(), "North 锚点应有非空值：{clone_v}");

    // 页 1 不该有克隆注入之外的意外行：North 块延续槽无值（未 emit），
    // 但 (1,0) 是锚点行自身，必有值。
    assert!(anchor_v.is_string(), "锚点值形态：{anchor_v}");
}
