//! 教程根主格：JSON 声明、方向约束、真实扩展和 RPX 往返。
use rpx_core::{open_bytes, to_bytes, ReportDefine};
use rpx_engine::{arena::CalcGrid, expr::ds::DataSet, Value};
use rpx_runtime::reportdef;
use serde_json::{json, Value as J};

fn build(doc: &J) -> ReportDefine {
    reportdef::build(&doc.to_string()).expect("模板应合法")
}

fn round_trip(rd: &ReportDefine) -> ReportDefine {
    let back = open_bytes(&to_bytes(rd).unwrap()).unwrap();
    assert_eq!(&back, rd, "根字符串与省略字段经过 RPX 后必须保持不同");
    back
}

#[test]
fn explicit_roots_are_preserved_and_omitted_heads_remain_automatic() {
    let rd = build(&json!({
        "grid": {"rows": 1, "cols": 4},
        "cells": [
            {"row": 1, "col": 1, "text": "左根", "left_head": " `0 "},
            {"row": 1, "col": 2, "text": "上根", "top_head": "`0"},
            {"row": 1, "col": 3, "text": "双根", "left_head": "`0", "top_head": "`0"},
            {"row": 1, "col": 4, "text": "自动"}
        ]
    }));
    let back = round_trip(&rd);
    for def in [&rd, &back] {
        let left = def.cell_at(1, 1).unwrap();
        assert_eq!(left.left_head.as_deref(), Some("`0"));
        assert_eq!(left.top_head, None);
        let top = def.cell_at(1, 2).unwrap();
        assert_eq!(top.left_head, None);
        assert_eq!(top.top_head.as_deref(), Some("`0"));
        let both = def.cell_at(1, 3).unwrap();
        assert_eq!(both.left_head.as_deref(), Some("`0"));
        assert_eq!(both.top_head.as_deref(), Some("`0"));
        let auto = def.cell_at(1, 4).unwrap();
        assert_eq!(
            (auto.left_head.as_deref(), auto.top_head.as_deref()),
            (None, None)
        );
    }
}

#[test]
fn a_root_is_allowed_on_either_axis_for_every_extension_direction() {
    for mode in ["default", "none", "vertical", "horizontal"] {
        let rd = build(&json!({
            "grid": {"rows": 1, "cols": 1},
            "cells": [{"row": 1, "col": 1, "expr": "to(1,3)", "extend_mode": mode,
                       "left_head": "`0", "top_head": "`0"}]
        }));
        let back = round_trip(&rd);
        assert_eq!(
            rpx_engine::calculate(&rd, &[]).unwrap(),
            rpx_engine::calculate(&back, &[]).unwrap()
        );
    }
}

#[test]
fn root_aliases_and_invalid_types_are_rejected_with_the_field_path() {
    for field in ["left_head", "top_head"] {
        for bad in [
            json!("0"),
            json!("root"),
            json!("ROOT"),
            json!("`00"),
            json!("``0"),
            json!(0),
            J::Null,
        ] {
            let mut cell = json!({"row": 1, "col": 1, "text": "x"});
            cell[field] = bad.clone();
            let doc = json!({"grid": {"rows": 1, "cols": 1}, "cells": [cell]});
            let error = reportdef::build(&doc.to_string()).unwrap_err().to_string();
            assert!(
                error.starts_with(&format!("$.cells[0].{field}")),
                "{bad}: {error}"
            );
        }
    }
}

fn piece(horizontal: bool, root: bool) -> J {
    let mut master = json!({"row": 1, "col": 1, "expr": "to(1,3)",
                           "extend_mode": if horizontal {"horizontal"} else {"vertical"}});
    // 根也可放在本方向禁止实际主格的那一轴。
    master[if horizontal { "left_head" } else { "top_head" }] = json!("`0");
    let mut child = json!({"row": if horizontal {2} else {1},
                          "col": if horizontal {1} else {2}, "text": "固定"});
    if root {
        child[if horizontal { "top_head" } else { "left_head" }] = json!("`0");
    }
    json!({"grid": {"rows": if horizontal {2} else {1},
                   "cols": if horizontal {1} else {2}}, "cells": [master, child]})
}

fn check_piece(horizontal: bool) {
    let automatic = build(&piece(horizontal, false));
    let rooted = build(&piece(horizontal, true));
    for (rd, root) in [(&automatic, false), (&rooted, true)] {
        let grid = rpx_engine::calculate(rd, &[]).unwrap();
        let back = round_trip(rd);
        assert_eq!(rpx_engine::calculate(&back, &[]).unwrap(), grid);
        assert_eq!(
            (grid.rows, grid.cols),
            if horizontal { (2, 3) } else { (3, 2) }
        );
        let (r, c) = if horizontal { (2, 1) } else { (1, 2) };
        let cell = grid.get(r, c).unwrap();
        assert_eq!(
            if horizontal {
                cell.top_head
            } else {
                cell.left_head
            },
            Some(if root { (0, 0) } else { (1, 1) })
        );
        if root {
            // 根格只被撑大为一个合并区；不能伪装成随主格复制的三个独立格。
            assert_eq!(grid.merged.len(), 3);
            for merged in &grid.merged {
                assert_eq!(
                    (merged.row_merge, merged.col_merge),
                    if horizontal { (1, 3) } else { (3, 1) }
                );
                assert_eq!(merged.value, Value::Text("固定".into()));
            }
        } else {
            assert!(grid.merged.is_empty(), "自动附属格应复制为三个独立格");
        }
        for n in 1..=3 {
            let (mr, mc) = if horizontal { (1, n) } else { (n, 1) };
            assert_eq!(grid.get(mr, mc).unwrap().value, Value::Int(n));
        }
    }
}

#[test]
fn left_root_stops_automatic_inheritance_and_vertical_copying() {
    check_piece(false);
}

#[test]
fn top_root_stops_automatic_inheritance_and_horizontal_copying() {
    check_piece(true);
}

fn grouped(horizontal: bool, root: bool) -> CalcGrid {
    let mut child = json!({"row": if horizontal {2} else {1},
                          "col": if horizontal {1} else {2}, "expr": "facts.count()", "extend_mode": "none"});
    if root {
        child[if horizontal { "top_head" } else { "left_head" }] = json!("`0");
    }
    let rd = build(&json!({
        "grid": {"rows": 2, "cols": 2}, "cells": [
            {"row": 1, "col": 1, "expr": "facts.group(category)",
             "extend_mode": if horizontal {"horizontal"} else {"vertical"}}, child
        ]
    }));
    let ds = DataSet::new(
        "facts",
        &["category"],
        ["A", "A", "B", "B"]
            .map(|v| vec![Value::Text(v.into())])
            .to_vec(),
    );
    let back = round_trip(&rd);
    let grid = rpx_engine::calculate(&rd, std::slice::from_ref(&ds)).unwrap();
    assert_eq!(rpx_engine::calculate(&back, &[ds]).unwrap(), grid);
    grid
}

#[test]
fn roots_stop_group_scope_inheritance_on_both_axes() {
    for horizontal in [false, true] {
        for root in [false, true] {
            let grid = grouped(horizontal, root);
            let origin = if horizontal { (2, 1) } else { (1, 2) };
            let cells: Vec<_> = grid.cells.iter().filter(|c| c.origin == origin).collect();
            assert!(!cells.is_empty());
            for cell in cells {
                assert_eq!(cell.value, Value::Int(if root { 4 } else { 2 }));
                assert_eq!(
                    if horizontal {
                        cell.top_head
                    } else {
                        cell.left_head
                    },
                    Some(if root { (0, 0) } else { (1, 1) })
                );
            }
        }
    }
}

#[test]
fn a_root_on_one_axis_does_not_disable_validation_of_the_other() {
    for (field, other, target_mode, own_mode, address, message) in [
        ("left_head", "top_head", "vertical", "none", "Z9", "不存在"),
        (
            "left_head",
            "top_head",
            "horizontal",
            "none",
            "B1",
            "左主格必须是纵向",
        ),
        (
            "top_head",
            "left_head",
            "vertical",
            "none",
            "B1",
            "上主格必须是横向",
        ),
        (
            "left_head",
            "top_head",
            "vertical",
            "horizontal",
            "B1",
            "横向扩展格不允许有左主格",
        ),
        (
            "top_head",
            "left_head",
            "horizontal",
            "vertical",
            "B1",
            "纵向扩展格不允许有上主格",
        ),
    ] {
        let mut child = json!({"row": 1, "col": 1, "text": "x", "extend_mode": own_mode});
        child[field] = json!(address);
        child[other] = json!("`0");
        let doc = json!({"grid": {"rows": 2, "cols": 2}, "cells": [child,
                        {"row": 1, "col": 2, "expr": "to(1,3)", "extend_mode": target_mode}]});
        let error = reportdef::build(&doc.to_string()).unwrap_err().to_string();
        assert!(error.starts_with(&format!("$.cells[0].{field}")), "{error}");
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn a_root_on_one_axis_does_not_hide_a_cycle_on_the_other() {
    for (field, other) in [("left_head", "top_head"), ("top_head", "left_head")] {
        let mut a = json!({"row": 1, "col": 1, "text": "a"});
        let mut b = json!({"row": 1, "col": 2, "text": "b"});
        a[field] = json!("B1");
        b[field] = json!("A1");
        a[other] = json!("`0");
        b[other] = json!("`0");
        let rd = build(&json!({"grid": {"rows": 1, "cols": 2}, "cells": [a, b]}));
        let error = rpx_engine::calculate(&rd, &[]).unwrap_err().to_string();
        assert!(error.contains("自声明成环"), "{field}: {error}");
    }
}

#[test]
fn json_and_rpx_render_the_same_rooted_and_automatic_results() {
    for horizontal in [false, true] {
        for root in [false, true] {
            let doc = piece(horizontal, root);
            let rd = build(&doc);
            let mut results = Vec::new();
            for report in [doc.to_string().into_bytes(), to_bytes(&rd).unwrap()] {
                let req = rpx_runtime::form::RenderRequest {
                    report,
                    report_name: "root-heads".into(),
                    datasets: Default::default(),
                    params: rpx_engine::ParamTable::EMPTY,
                };
                let text = rpx_runtime::execution::render_request(&req).unwrap();
                results.push(serde_json::from_str::<J>(&text).unwrap());
            }
            assert_eq!(results[0], results[1], "JSON 与 RPX 入口的运行结果应一致");
            let sheet = &results[0]["sheets"]["rpx-sheet-1"];
            let merges = sheet["mergeData"].as_array().unwrap();
            assert_eq!(merges.len(), usize::from(root));
            if root {
                assert_eq!(
                    merges[0],
                    if horizontal {
                        json!({"startRow": 1, "endRow": 1, "startColumn": 0, "endColumn": 2})
                    } else {
                        json!({"startRow": 0, "endRow": 2, "startColumn": 1, "endColumn": 1})
                    }
                );
            }
            for n in 0..3 {
                let (r, c) = if horizontal { (1, n) } else { (n, 1) };
                assert_eq!(sheet["cellData"][r.to_string()][c.to_string()]["v"], "固定");
            }
        }
    }
}
