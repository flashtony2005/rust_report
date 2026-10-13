//! 段级比对：`pagination` 与 `merged`。
//!
//! 两者都是**分页后**的产物（`CalcMergeJson.java` / `CalcJson.java:113-143`），
//! 坐标是**页内**坐标（已由 `tests/fixtures/ProbeMergePages.java` 在 10 个页面上实测确认）。
//! 这里按夹具写下来的值逐项比，**不**推断任何布局规则。

use super::super::json::Json;
use super::util::{merged_index, page_rows, raw_of, union_keys};
use super::types::{Diff, Dim, Kind};

/// 取段；`null` / 缺键 ⇒ `None`。
fn opt_section<'a>(root: &'a Json, key: &str) -> Option<&'a Json> {
    root.get(key).filter(|v| !v.is_null())
}

pub(super) fn compare_pagination(d: &mut Vec<Diff>, actual: &Json, expected: &Json) {
    let (ep, ap) = (opt_section(expected, "pagination"), opt_section(actual, "pagination"));
    match (ep, ap) {
        (Some(e), Some(a)) => {
            for key in ["page_count", "x_pages", "y_pages", "row_num_per_page"] {
                push_if_diff(d, Dim::Pagination, key, e, a, -1, -1);
            }
            // 逐页 rows：按页序比，长度不同按 missing/extra 记
            let er = page_rows(e);
            let ar = page_rows(a);
            for i in 0..er.len().max(ar.len()) {
                let (ev, av) = (
                    er.get(i).cloned().unwrap_or_else(|| "<无该页>".into()),
                    ar.get(i).cloned().unwrap_or_else(|| "<无该页>".into()),
                );
                if ev != av {
                    d.push(Diff {
                        kind: if i >= er.len() {
                            Kind::Extra
                        } else if i >= ar.len() {
                            Kind::Missing
                        } else {
                            Kind::Mismatch
                        },
                        r: -1,
                        c: -1,
                        dim: Dim::Pagination,
                        detail: format!("page[{}].rows", i + 1),
                        expected: ev,
                        actual: av,
                    });
                }
            }
        }
        (Some(_), None) => d.push(Diff {
            kind: Kind::Missing,
            r: -1,
            c: -1,
            dim: Dim::Pagination,
            detail: "pagination 段在引擎输出中缺席".to_string(),
            expected: "有".into(),
            actual: "无".into(),
        }),
        (None, Some(_)) => d.push(Diff {
            kind: Kind::Extra,
            r: -1,
            c: -1,
            dim: Dim::Pagination,
            detail: "引擎多产出了 pagination 段".to_string(),
            expected: "无".into(),
            actual: "有".into(),
        }),
        (None, None) => {}
    }
}

pub(super) fn compare_merged(d: &mut Vec<Diff>, actual: &Json, expected: &Json) {
    let (em, am) = (opt_section(expected, "merged"), opt_section(actual, "merged"));
    let (Some(e), Some(a)) = (em, am) else {
        return;
    };
    push_if_diff(d, Dim::Merged, "page_count", e, a, -1, -1);
    // 合并格按 (page, r, c) 建索引 —— r/c 是**页内**坐标，page 参与主键
    let ei = merged_index(e.get("cells"));
    let ai = merged_index(a.get("cells"));
    for k in union_keys(&ei, &ai) {
        let (page, r, c) = k;
        match (ei.get(&k), ai.get(&k)) {
            (Some(ec), Some(ac)) => {
                for key in ["value", "row_merge", "col_merge"] {
                    push_if_diff(d, Dim::Merged, &format!("page{page}.{key}"), ec, ac, r, c);
                }
            }
            (Some(_), None) => d.push(Diff {
                kind: Kind::Missing,
                r,
                c,
                dim: Dim::Merged,
                detail: format!("第 {page} 页的合并格缺席"),
                expected: "有".into(),
                actual: String::new(),
            }),
            (None, Some(_)) => d.push(Diff {
                kind: Kind::Extra,
                r,
                c,
                dim: Dim::Merged,
                detail: format!("第 {page} 页的合并格多出"),
                expected: String::new(),
                actual: "有".into(),
            }),
            (None, None) => unreachable!("坐标来自两侧并集"),
        }
    }
}

fn push_if_diff(d: &mut Vec<Diff>, dim: Dim, key: &str, e: &Json, a: &Json, r: i32, c: i32) {
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
