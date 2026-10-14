//! 逐格比对：`calculated.cells[]` 与 `source.sub_values[]`。
//!
//! 纪律 ②（稀疏格不算缺失）、③（`source` 可选、calc 坐标 ≠ src 坐标）都在这里落地。
//! 值一律按**原文 token** 比，不做数值归一化（M2-14：`100` ≠ `100.0`）。

use super::super::json::Json;
use super::types::{Diff, Dim, Kind};
use super::util::{cell_index, raw_of, sub_index, union_keys};

/// 比对 `calculated` 段：`act_calc` = 引擎侧，`exp_calc` = 夹具侧。
pub(super) fn compare_cells(d: &mut Vec<Diff>, act_calc: &Json, exp_calc: &Json) {
    let exp_idx = cell_index(exp_calc.get("cells"));
    let act_idx = cell_index(act_calc.get("cells"));
    for rc in union_keys(&exp_idx, &act_idx) {
        let (r, c) = rc;
        match (exp_idx.get(&rc), act_idx.get(&rc)) {
            (Some(e), Some(a)) => compare_one_cell(d, rc, a, e),
            (Some(e), None) => d.push(Diff {
                kind: Kind::Missing,
                r,
                c,
                dim: Dim::Value,
                detail: "整个格在引擎输出中缺席".to_string(),
                expected: raw_of(e.get("value")),
                actual: String::new(),
            }),
            (None, Some(a)) => d.push(Diff {
                kind: Kind::Extra,
                r,
                c,
                dim: Dim::Value,
                detail: "整个格是引擎多出来的".to_string(),
                expected: String::new(),
                actual: raw_of(a.get("value")),
            }),
            (None, None) => unreachable!("坐标来自两侧并集"),
        }
    }
}

fn compare_one_cell(d: &mut Vec<Diff>, rc: (i32, i32), a: &Json, e: &Json) {
    let (r, c) = rc;
    // 值与类型：原文 token 比（Dim::Type 独立成条，便于"值对但类型错"被单独看见）
    for (dim, key) in [(Dim::Value, "value"), (Dim::Type, "type")] {
        let ev = raw_of(e.get(key));
        let av = raw_of(a.get(key));
        if ev != av {
            d.push(Diff {
                kind: Kind::Mismatch,
                r,
                c,
                dim,
                detail: String::new(),
                expected: ev,
                actual: av,
            });
        }
    }
    // disp_value：两侧都是可选键（`CalcJson.java:71-73` 按条件才写）
    let ed = e.get("disp_value").map(|v| v.raw.clone());
    let ad = a.get("disp_value").map(|v| v.raw.clone());
    if ed != ad {
        d.push(Diff {
            kind: Kind::Mismatch,
            r,
            c,
            dim: Dim::DispValue,
            detail: String::new(),
            expected: ed.unwrap_or_else(|| "<无此键>".into()),
            actual: ad.unwrap_or_else(|| "<无此键>".into()),
        });
    }
    compare_head(d, rc, Dim::LeftHead, a.get("left_head"), e.get("left_head"));
    compare_head(d, rc, Dim::TopHead, a.get("top_head"), e.get("top_head"));
    compare_source(d, rc, a, e);
}

/// `left_head` / `top_head`：`{src_row, src_col}`，**模板坐标**。
///
/// `(0,0)` 唯一表示 `cell00`（裸数字表达不了这个约定，见 `HeadRef` 的文档），
/// 所以这里逐字段比 `src_row` / `src_col`，不按对象原文比 —— 否则键序不同就会假不一致。
/// 两侧都缺键 ⇒ 无差异（模型里是 `Option`）。
fn compare_head(
    d: &mut Vec<Diff>,
    rc: (i32, i32),
    dim: Dim,
    actual: Option<&Json>,
    expected: Option<&Json>,
) {
    let (r, c) = rc;
    let (e, a) = match (expected, actual) {
        (Some(e), Some(a)) => (e, a),
        (None, None) => return,
        (Some(e), None) => {
            d.push(Diff {
                kind: Kind::Missing,
                r,
                c,
                dim,
                detail: "夹具带该 head 引用，引擎未产出".to_string(),
                expected: head_repr(e),
                actual: String::new(),
            });
            return;
        }
        (None, Some(a)) => {
            d.push(Diff {
                kind: Kind::Extra,
                r,
                c,
                dim,
                detail: "引擎多产出了该 head 引用".to_string(),
                expected: String::new(),
                actual: head_repr(a),
            });
            return;
        }
    };
    for key in ["src_row", "src_col"] {
        let ev = raw_of(e.get(key));
        let av = raw_of(a.get(key));
        if ev != av {
            d.push(Diff {
                kind: Kind::Mismatch,
                r,
                c,
                dim,
                detail: key.to_string(),
                expected: ev,
                actual: av,
            });
        }
    }
}

fn head_repr(v: &Json) -> String {
    format!(
        "({}, {})",
        raw_of(v.get("src_row")),
        raw_of(v.get("src_col"))
    )
}

/// `source` 是**可选键**（纪律 ③）。
///
/// ⛔ 这里**不**检查「calc 坐标是否等于 `src_row`/`src_col`」—— `engine_merge` 的
/// `(2,4)` 的 `src` 就是 `(2,3)`，那条"不变量"在真实数据上不成立。只按夹具写的值比。
fn compare_source(d: &mut Vec<Diff>, rc: (i32, i32), a: &Json, e: &Json) {
    let (r, c) = rc;
    match (e.get("source"), a.get("source")) {
        (Some(es), Some(asc)) => {
            for key in [
                "src_row",
                "src_col",
                "row_merge",
                "col_merge",
                "extend",
                "sub_cells",
            ] {
                let ev = raw_of(es.get(key));
                let av = raw_of(asc.get(key));
                if ev != av {
                    d.push(Diff {
                        kind: Kind::Mismatch,
                        r,
                        c,
                        dim: Dim::Source,
                        detail: key.to_string(),
                        expected: ev,
                        actual: av,
                    });
                }
            }
            compare_sub_values(d, rc, asc.get("sub_values"), es.get("sub_values"));
        }
        (Some(es), None) => d.push(Diff {
            kind: Kind::Missing,
            r,
            c,
            dim: Dim::Source,
            detail: "夹具带 source，引擎未产出".to_string(),
            expected: raw_of(es.get("src_row")),
            actual: String::new(),
        }),
        (None, Some(asc)) => d.push(Diff {
            kind: Kind::Extra,
            r,
            c,
            dim: Dim::Source,
            detail: "引擎多产出了 source".to_string(),
            expected: String::new(),
            actual: raw_of(asc.get("src_row")),
        }),
        (None, None) => {}
    }
}

fn compare_sub_values(d: &mut Vec<Diff>, rc: (i32, i32), act: Option<&Json>, exp: Option<&Json>) {
    let (r, c) = rc;
    let exp_idx = sub_index(exp);
    let act_idx = sub_index(act);
    for k in union_keys(&exp_idx, &act_idx) {
        match (exp_idx.get(&k), act_idx.get(&k)) {
            (Some(e), Some(a)) => {
                for (dim, key) in [(Dim::SubValue, "value"), (Dim::SubValue, "type")] {
                    let ev = raw_of(e.get(key));
                    let av = raw_of(a.get(key));
                    if ev != av {
                        d.push(Diff {
                            kind: Kind::Mismatch,
                            r,
                            c,
                            dim,
                            detail: format!("sub_value({},{}).{key}", k.0, k.1),
                            expected: ev,
                            actual: av,
                        });
                    }
                }
            }
            (Some(e), None) => d.push(Diff {
                kind: Kind::Missing,
                r,
                c,
                dim: Dim::SubValue,
                detail: format!("sub_value({0},{1})", k.0, k.1),
                expected: raw_of(e.get("value")),
                actual: String::new(),
            }),
            (None, Some(a)) => d.push(Diff {
                kind: Kind::Extra,
                r,
                c,
                dim: Dim::SubValue,
                detail: format!("sub_value({0},{1})", k.0, k.1),
                expected: String::new(),
                actual: raw_of(a.get("value")),
            }),
            (None, None) => unreachable!("坐标来自两侧并集"),
        }
    }
}
