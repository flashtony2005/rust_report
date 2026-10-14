//! 段级比对：`pagination` 与 `merged`。
//!
//! 两者都是**分页后**的产物（`CalcMergeJson.java` / `CalcJson.java:113-143`），
//! 坐标是**页内**坐标（已由 `tests/fixtures/ProbeMergePages.java` 在 10 个页面上实测确认）。
//! 这里按夹具写下来的值逐项比，**不**推断任何布局规则。

use super::super::json::Json;
use super::types::{Diff, Dim, Kind};
use super::util::{merged_index, page_list, raw_of, union_keys};

/// 取段；`null` / 缺键 ⇒ `None`。
fn opt_section<'a>(root: &'a Json, key: &str) -> Option<&'a Json> {
    root.get(key).filter(|v| !v.is_null())
}

/// `pagination` 段的标量键（键序与 `CalcJson.java:177-185` 同）。
const PAG_SCALARS: [&str; 7] = [
    "paper_w",
    "paper_h",
    "columns",
    "page_count",
    "x_pages",
    "y_pages",
    "row_num_per_page",
];

/// 每页的标量键（`page` 由页序隐含，不另比）。
const PAGE_SCALARS: [&str; 4] = ["rows", "first_value", "last_value", "col1_values"];

pub(super) fn compare_pagination(d: &mut Vec<Diff>, actual: &Json, expected: &Json) {
    let (ep, ap) = (
        opt_section(expected, "pagination"),
        opt_section(actual, "pagination"),
    );
    match (ep, ap) {
        (Some(e), Some(a)) => {
            for key in PAG_SCALARS {
                push_if_diff(d, Dim::Pagination, DiffLoc::new(key, -1, -1), key, e, a);
            }
            // 逐页比**全部**观测值：`rows`（含页首区带与分组头副本）、
            // `first_value` / `last_value`（`String.valueOf`）、`col1_values`（带类型的原文 token）。
            // ⛔ 按页序比，长度不同按 missing/extra 记 —— 页数对不上时这里会成串报出来。
            let (er, ar) = (page_list(e), page_list(a));
            for i in 0..er.len().max(ar.len()) {
                let kind = if i >= er.len() {
                    Kind::Extra
                } else if i >= ar.len() {
                    Kind::Missing
                } else {
                    Kind::Mismatch
                };
                match (er.get(i), ar.get(i)) {
                    (Some(ev), Some(av)) => {
                        for key in PAGE_SCALARS {
                            push_if_diff(
                                d,
                                Dim::Pagination,
                                DiffLoc::new(&format!("page[{}].{key}", i + 1), -1, -1),
                                key,
                                ev,
                                av,
                            );
                        }
                    }
                    (ev, av) => {
                        // 一侧缺该页：整页记一条（不逐键报，避免淹没首错）
                        d.push(Diff {
                            kind,
                            r: -1,
                            c: -1,
                            dim: Dim::Pagination,
                            detail: format!("page[{}]", i + 1),
                            expected: ev.map(|p| raw_of(p.get("rows"))).unwrap_or_default(),
                            actual: av.map(|p| raw_of(p.get("rows"))).unwrap_or_default(),
                        });
                    }
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
    let (em, am) = (
        opt_section(expected, "merged"),
        opt_section(actual, "merged"),
    );
    let (Some(e), Some(a)) = (em, am) else {
        return;
    };
    // `page_count` 单列一维：它属分页器（M2-11），而下面的格级差异属合并算法（已落地）。
    push_if_diff(
        d,
        Dim::MergedPageCount,
        DiffLoc::new("page_count", -1, -1),
        "page_count",
        e,
        a,
    );
    // 合并格按 (page, r, c) 建索引 —— r/c 是**页内**坐标，page 参与主键
    let ei = merged_index(e.get("cells"));
    let ai = merged_index(a.get("cells"));
    for k in union_keys(&ei, &ai) {
        let (page, r, c) = k;
        match (ei.get(&k), ai.get(&k)) {
            (Some(ec), Some(ac)) => {
                for key in ["value", "row_merge", "col_merge"] {
                    push_if_diff(
                        d,
                        Dim::Merged,
                        DiffLoc::new(&format!("page{page}.{key}"), r, c),
                        key,
                        ec,
                        ac,
                    );
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

/// 一个差异的位置与显示标签；把它与真正的 JSON `key` 分开传。
struct DiffLoc<'a> {
    label: &'a str,
    r: i32,
    c: i32,
}

impl<'a> DiffLoc<'a> {
    fn new(label: &'a str, r: i32, c: i32) -> Self {
        DiffLoc { label, r, c }
    }
}

/// 比 `e[key]` 与 `a[key]`，不同则记一条差异。
///
/// ⛔ **`loc.label` 与 `key` 必须分开**（2026-10-13 由故障注入发现的**静默恒真**）：
/// 早先只有 `key` 一个参数、同时当"显示名"与"JSON 键"用。逐页/逐格那两处传的是
/// `page[2].rows`、`page1.value` 这类**标签**，拿它去 `get` 两边都取不到 ⇒
/// 都变成 [`raw_of`] 的 `"<无此键>"` ⇒ **整段比较恒真、从未真正比过**。
/// 逐页的 `rows`/`first_value`/`last_value`/`col1_values` 与合并格的
/// `value`/`row_merge`/`col_merge` 都曾落在这个洞里。
fn push_if_diff(d: &mut Vec<Diff>, dim: Dim, loc: DiffLoc<'_>, key: &str, e: &Json, a: &Json) {
    let ev = raw_of(e.get(key));
    let av = raw_of(a.get(key));
    if ev != av {
        d.push(Diff {
            kind: Kind::Mismatch,
            r: loc.r,
            c: loc.c,
            dim,
            detail: loc.label.to_string(),
            expected: ev,
            actual: av,
        });
    }
}
