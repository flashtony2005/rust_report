//! M1 黄金测试：3 个真实 .rpx 夹具 vs Java 侧 expected.json。
//!
//! 对拍规则（RUNLOG §4）：
//! - 缺失字段 = 模型默认值（契约附件 B/D 默认值列）；
//! - cells 仅收录非默认属性格；合并区仅锚点记录；
//! - 浮点按 1e-5 容差（Java float 经 f64 打印 vs Rust f32）；
//! - 合并区网格各处持同一 CellId（AC-04）。

mod common;

use common::J;
use rpx_core::reader::ObjValue;
use std::collections::HashSet;

const TOL: f64 = 1e-5;

macro_rules! load {
    ($name:literal) => {{
        let rpx: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/",
            $name,
            ".rpx"
        ));
        let json = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/",
            $name,
            ".expected.json"
        ));
        (rpx, common::parse(json))
    }};
}

fn check(name: &str, rpx: &[u8], j: J) {
    let r = rpx_core::open_bytes(rpx).unwrap_or_else(|e| panic!("{}: parse failed: {}", name, e));

    // ---- 维度 ----
    let rows = j.get("rows").unwrap().as_i64().unwrap() as usize;
    let cols = j.get("cols").unwrap().as_i64().unwrap() as usize;
    assert_eq!(r.rows, rows, "{}: rows", name);
    assert_eq!(r.cols, cols, "{}: cols", name);
    assert_eq!(r.row_cells.len(), rows - 1, "{}: row_cells", name);
    assert_eq!(r.col_cells.len(), cols - 1, "{}: col_cells", name);

    // ---- 行高列宽 ----
    let rh: Vec<f64> = j
        .get("row_heights")
        .unwrap()
        .as_arr()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let cw: Vec<f64> = j
        .get("col_widths")
        .unwrap()
        .as_arr()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert_eq!(rh.len(), rows - 1, "{}: row_heights len", name);
    assert_eq!(cw.len(), cols - 1, "{}: col_widths len", name);
    for (i, want) in rh.iter().enumerate() {
        let got = r.row_height(i + 1).unwrap() as f64;
        assert!(
            (got - want).abs() < TOL,
            "{}: row_height[{}] got {} want {}",
            name,
            i + 1,
            got,
            want
        );
    }
    for (i, want) in cw.iter().enumerate() {
        let got = r.col_width(i + 1).unwrap() as f64;
        assert!(
            (got - want).abs() < TOL,
            "{}: col_width[{}] got {} want {}",
            name,
            i + 1,
            got,
            want
        );
    }

    // ---- 单元格 ----
    let json_cells = j.get("cells").unwrap().as_arr().unwrap();
    let mut expected_keys: HashSet<(usize, usize)> = HashSet::new();
    for jc in json_cells {
        let row = jc.get("row").unwrap().as_i64().unwrap() as usize;
        let col = jc.get("col").unwrap().as_i64().unwrap() as usize;
        expected_keys.insert((row, col));
        let cell = r
            .cell_at(row, col)
            .unwrap_or_else(|| panic!("{}: cell ({},{}) is empty", name, row, col));

        // value（类型敏感）
        if let Some(v) = jc.get("value") {
            match v {
                J::Str(s) => assert_eq!(
                    &cell.value,
                    &ObjValue::Str(s.clone()),
                    "{}: ({},{}) value",
                    name,
                    row,
                    col
                ),
                J::Num(n) => {
                    if n.contains('.') || n.contains('e') || n.contains('E') {
                        let want: f64 = n.parse().unwrap();
                        match &cell.value {
                            ObjValue::Double(d) => assert!(
                                (*d - want).abs() < TOL,
                                "{}: ({},{}) double",
                                name,
                                row,
                                col
                            ),
                            ObjValue::Float(f) => assert!(
                                (*f as f64 - want).abs() < TOL,
                                "{}: ({},{}) float",
                                name,
                                row,
                                col
                            ),
                            other => {
                                panic!("{}: ({},{}) want double got {:?}", name, row, col, other)
                            }
                        }
                    } else {
                        let want: i64 = n.parse().unwrap();
                        match &cell.value {
                            ObjValue::Int(i) => {
                                assert_eq!(*i as i64, want, "{}: ({},{}) int", name, row, col)
                            }
                            ObjValue::Long(l) => {
                                assert_eq!(*l, want, "{}: ({},{}) long", name, row, col)
                            }
                            other => panic!("{}: ({},{}) want int got {:?}", name, row, col, other),
                        }
                    }
                }
                J::Bool(b) => assert_eq!(
                    &cell.value,
                    &ObjValue::Bool(*b),
                    "{}: ({},{}) bool",
                    name,
                    row,
                    col
                ),
                J::Null => assert_eq!(
                    cell.value,
                    ObjValue::Null,
                    "{}: ({},{}) null",
                    name,
                    row,
                    col
                ),
                _ => panic!("{}: ({},{}) unexpected value JSON", name, row, col),
            }
        }

        // dispValue
        if let Some(dv) = jc.get("dispValue") {
            assert_eq!(
                cell.disp_value.as_deref(),
                dv.as_str(),
                "{}: ({},{}) dispValue",
                name,
                row,
                col
            );
        }
        // extendMode
        if let Some(em) = jc.get("extendMode") {
            assert_eq!(
                cell.extend_mode as i64,
                em.as_i64().unwrap(),
                "{}: ({},{}) extendMode",
                name,
                row,
                col
            );
        }
        // 对齐
        if let Some(h) = jc.get("hAlign") {
            assert_eq!(
                cell.h_align as i64,
                h.as_i64().unwrap(),
                "{}: ({},{}) hAlign",
                name,
                row,
                col
            );
        }
        if let Some(v) = jc.get("vAlign") {
            assert_eq!(
                cell.v_align as i64,
                v.as_i64().unwrap(),
                "{}: ({},{}) vAlign",
                name,
                row,
                col
            );
        }
        // 字体
        if let Some(f) = jc.get("fontName") {
            assert_eq!(
                cell.font_name.as_deref(),
                f.as_str(),
                "{}: ({},{}) fontName",
                name,
                row,
                col
            );
        }
        if let Some(fs) = jc.get("fontSize") {
            assert_eq!(
                cell.font_size as i64,
                fs.as_i64().unwrap(),
                "{}: ({},{}) fontSize",
                name,
                row,
                col
            );
        }
        // 位标志
        if let Some(b) = jc.get("bold") {
            assert_eq!(
                cell.bold(),
                b.as_bool().unwrap(),
                "{}: ({},{}) bold",
                name,
                row,
                col
            );
        }
        if let Some(b) = jc.get("italic") {
            assert_eq!(
                cell.italic(),
                b.as_bool().unwrap(),
                "{}: ({},{}) italic",
                name,
                row,
                col
            );
        }
        if let Some(b) = jc.get("underline") {
            assert_eq!(
                cell.underline(),
                b.as_bool().unwrap(),
                "{}: ({},{}) underline",
                name,
                row,
                col
            );
        }
        if let Some(b) = jc.get("textWrap") {
            assert_eq!(
                cell.text_wrap(),
                b.as_bool().unwrap(),
                "{}: ({},{}) textWrap",
                name,
                row,
                col
            );
        }
        if let Some(b) = jc.get("visible") {
            assert_eq!(
                cell.visible(),
                b.as_bool().unwrap(),
                "{}: ({},{}) visible",
                name,
                row,
                col
            );
        }
        // 颜色
        if let Some(c) = jc.get("foreColor") {
            assert_eq!(
                cell.fore_color as i64,
                c.as_i64().unwrap(),
                "{}: ({},{}) foreColor",
                name,
                row,
                col
            );
        }
        if let Some(c) = jc.get("backColor") {
            assert_eq!(
                cell.back_color as i64,
                c.as_i64().unwrap(),
                "{}: ({},{}) backColor",
                name,
                row,
                col
            );
        }
        // 其他样式
        if let Some(f) = jc.get("format") {
            assert_eq!(
                cell.format.as_deref(),
                f.as_str(),
                "{}: ({},{}) format",
                name,
                row,
                col
            );
        }
        if let Some(ra) = jc.get("rotationAngle") {
            assert!(
                (cell.rotation_angle as f64 - ra.as_f64().unwrap()).abs() < TOL,
                "{}: ({},{}) rotationAngle",
                name,
                row,
                col
            );
        }
        if let Some(ind) = jc.get("indent") {
            assert!(
                (cell.indent as f64 - ind.as_f64().unwrap()).abs() < TOL,
                "{}: ({},{}) indent",
                name,
                row,
                col
            );
        }
        // 合并区
        if let Some(m) = jc.get("merged") {
            let area = cell
                .merged_area
                .unwrap_or_else(|| panic!("{}: ({},{}) merged missing", name, row, col));
            assert_eq!(
                area.begin_row as i64,
                m.get("begin_row").unwrap().as_i64().unwrap(),
                "{}: merged begin_row",
                name
            );
            assert_eq!(
                area.begin_col as i64,
                m.get("begin_col").unwrap().as_i64().unwrap(),
                "{}: merged begin_col",
                name
            );
            assert_eq!(
                area.end_row as i64,
                m.get("end_row").unwrap().as_i64().unwrap(),
                "{}: merged end_row",
                name
            );
            assert_eq!(
                area.end_col as i64,
                m.get("end_col").unwrap().as_i64().unwrap(),
                "{}: merged end_col",
                name
            );
        }
    }

    // ---- 合并区双向校验（AC-04）+ 非默认格完备性 ----
    // 独立验证发现旧版此处自证：covered 由解析侧 r.merges 构建，锚点 merged 又是可选断言，
    // 删掉 JSON 里的合并声明测试仍全绿（变异 M3）。现改为双向对拍（common::merge_checks）。
    let covered = common::merge_checks::check_merges(name, &r, &j);
    for row in 1..r.rows {
        for col in 1..r.cols {
            if covered.contains(&(row, col)) {
                continue;
            }
            if !r.is_blank(row, col) {
                assert!(
                    expected_keys.contains(&(row, col)),
                    "{}: non-default cell ({},{}) missing from expected.json",
                    name,
                    row,
                    col
                );
            }
        }
    }

    // ---- MetaData 段（params/datasets/print_setup/scalars） ----
    common::meta_checks::check_meta(name, &r, &j);
}

#[test]
fn golden_simple() {
    let (rpx, j) = load!("simple");
    check("simple", rpx, j);
}

#[test]
fn golden_merged() {
    let (rpx, j) = load!("merged");
    check("merged", rpx, j);
}

#[test]
fn golden_full() {
    let (rpx, j) = load!("full");
    check("full", rpx, j);
}

/// **真合并**夹具（M1 遗留项①的闭合证据）：合并非锚点在流里是 `int32 -1` 空记录，
/// 不是完整默认记录。区别在解析结果上可判别——`merged.rpx`（浅合并）的非锚点槽持自己的
/// 默认格，而本夹具的非锚点槽**全部持锚点 id**（`report.rs` 的 `-1` 空槽分支被走到）。
#[test]
fn golden_realmerge() {
    let (rpx, j) = load!("realmerge");
    check("realmerge", rpx, j);
}

// ---------- 错误路径（AC-01/02/09） ----------

#[test]
fn rejects_bad_magic() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"XXXX");
    bytes.extend_from_slice(&[0, 0, 0, 8]);
    bytes.extend_from_slice(&[0u8; 16]);
    let err = rpx_core::open_bytes(&bytes).unwrap_err();
    assert!(
        matches!(err, rpx_core::RpxError::UnsupportedFormat),
        "got {:?}",
        err
    );
}

#[test]
fn rejects_truncated_payload() {
    let (rpx, _) = load!("simple");
    let cut = &rpx[..8 + 100]; // 声明 3610，实际只有 100
    let err = rpx_core::open_bytes(cut).unwrap_err();
    assert!(
        matches!(err, rpx_core::RpxError::Truncated { .. }),
        "got {:?}",
        err
    );
}

#[test]
fn rejects_bad_footer() {
    let (rpx, _) = load!("simple");
    let mut bytes = rpx.to_vec();
    let n = bytes.len();
    bytes[n - 1] ^= 0xFF; // 破坏页脚
    let err = rpx_core::open_bytes(&bytes).unwrap_err();
    assert!(
        matches!(err, rpx_core::RpxError::BadFooter),
        "got {:?}",
        err
    );
}

/// 健壮性扫掠：三个夹具的每个前缀长度都必须优雅返回 Err，绝不 panic。
/// （截断点覆盖文件头/长度域/各记录边界/页脚前）
#[test]
fn never_panics_on_any_truncation() {
    let fixtures: [(&str, &[u8]); 4] = [
        ("simple", load!("simple").0),
        ("merged", load!("merged").0),
        ("full", load!("full").0),
        ("realmerge", load!("realmerge").0),
    ];
    for (name, rpx) in fixtures {
        for cut in 0..rpx.len() {
            let err = rpx_core::open_bytes(&rpx[..cut]);
            assert!(
                err.is_err(),
                "{}: prefix of {} bytes parsed successfully (must be Truncated)",
                name,
                cut
            );
        }
        // 完整文件必须成功
        assert!(
            rpx_core::open_bytes(rpx).is_ok(),
            "{}: full file must parse",
            name
        );
    }
}
