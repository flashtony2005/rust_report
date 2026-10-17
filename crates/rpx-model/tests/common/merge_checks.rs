//! 合并区对拍（AC-04）：expected.json 声明 ↔ 解析结果**双向**校验。
//!
//! 独立验证（2026-10-12）发现旧版此处是单向自证，三处叠加导致合并区完全失去校验：
//! 覆盖集合 `covered` 由解析侧 `r.merges` 构建而 `is_blank` 也来自解析侧（循环论证）；
//! 锚点的 `merged` 键用 `if let Some(...)` 断言，键缺失即静默跳过；
//! 且从不校验 `r.merges.len()` 与 JSON 声明的数量。
//!
//! 后果（变异测试 M3，已确认）：删掉锚点格 JSON 里的 `merged` 键、甚至删光全部三个，
//! 7 个 golden 测试仍然全绿 —— 解析器丢掉整个合并区也测不出来。
//!
//! 本模块从 JSON 侧独立重建合并区清单，与 `r.merges` 做数量 + 坐标 + 锚点 CellId
//! 双向对齐；覆盖集合也改由 JSON 声明构建，切断自证循环。

use super::J;
use rpx_core::ReportDefine;
use std::collections::HashSet;

/// 合并区四端点 `(begin_row, begin_col, end_row, end_col)`。
pub type MergeRect = (usize, usize, usize, usize);

/// 从 expected.json 的 cells 数组重建合并区清单。
/// 锚点判定：`merged.begin_row/begin_col` 与该格自身坐标相同（writer 只给锚点写 merged）。
pub fn json_merges(j: &J) -> Vec<MergeRect> {
    let mut out = Vec::new();
    let Some(cells) = j.get("cells").and_then(|v| v.as_arr()) else {
        return out;
    };
    for jc in cells {
        let (Some(row), Some(col)) = (
            jc.get("row").and_then(|v| v.as_i64()),
            jc.get("col").and_then(|v| v.as_i64()),
        ) else {
            continue;
        };
        let Some(m) = jc.get("merged") else { continue };
        let (Some(br), Some(bc)) = (
            m.get("begin_row").and_then(|v| v.as_i64()),
            m.get("begin_col").and_then(|v| v.as_i64()),
        ) else {
            panic!("merged without begin_row/begin_col");
        };
        if (br, bc) != (row, col) {
            continue; // 非锚点格即便带 merged 也不计入清单
        }
        out.push((
            br as usize,
            bc as usize,
            m.get("end_row").unwrap().as_i64().unwrap() as usize,
            m.get("end_col").unwrap().as_i64().unwrap() as usize,
        ));
    }
    out
}

fn find(r: &ReportDefine, rect: &MergeRect) -> Option<rpx_core::MergedArea> {
    r.merges
        .iter()
        .find(|m| m.begin_row == rect.0 && m.begin_col == rect.1)
        .copied()
}

/// 双向校验合并区，返回「被合并区覆盖的非锚点槽」集合（供非默认格完备性检查排除）。
pub fn check_merges(name: &str, r: &ReportDefine, j: &J) -> HashSet<(usize, usize)> {
    let jm = json_merges(j);

    // ---- 方向一：JSON 声明的每个合并区都必须在解析结果里存在 ----
    assert_eq!(
        r.merges.len(),
        jm.len(),
        "{}: merge count mismatch — expected.json declares {}, parser produced {}",
        name,
        jm.len(),
        r.merges.len()
    );
    for rect in &jm {
        let m = find(r, rect).unwrap_or_else(|| {
            panic!(
                "{}: merge ({},{})..({},{}) declared in expected.json but missing from parser output",
                name, rect.0, rect.1, rect.2, rect.3
            )
        });
        assert_eq!(
            (m.end_row, m.end_col),
            (rect.2, rect.3),
            "{}: merge ({},{}) span",
            name,
            rect.0,
            rect.1
        );
        assert_eq!(
            r.grid[rect.0][rect.1],
            Some(m.cell),
            "{}: merge ({},{}) anchor slot must hold the merged cell",
            name,
            rect.0,
            rect.1
        );
    }

    // ---- 方向二：解析出的每个合并区都必须被 expected.json 声明 ----
    for m in &r.merges {
        assert!(
            jm.contains(&(m.begin_row, m.begin_col, m.end_row, m.end_col)),
            "{}: parsed merge ({},{})..({},{}) absent from expected.json",
            name,
            m.begin_row,
            m.begin_col,
            m.end_row,
            m.end_col
        );
    }

    // ---- 网格语义（Java fillRecord 忠实语义，ReportDefine.java:1143-1161）----
    // 锚点铺满整区；区内其他槽要么持锚点 id（流里是 -1 空记录），要么是完整默认记录。
    for m in &r.merges {
        for sr in m.begin_row..=m.end_row {
            for sc in m.begin_col..=m.end_col {
                if (sr, sc) == (m.begin_row, m.begin_col) {
                    continue;
                }
                match r.grid[sr][sc] {
                    Some(id) if id == m.cell => {}
                    Some(id) => assert!(
                        r.store[id].is_default(),
                        "{}: merge ({},{})..({},{}) slot ({},{}) must be anchor or default cell",
                        name,
                        m.begin_row,
                        m.begin_col,
                        m.end_row,
                        m.end_col,
                        sr,
                        sc
                    ),
                    None => panic!(
                        "{}: merge slot ({},{}) is empty (writer writes either -1 or a full record)",
                        name, sr, sc
                    ),
                }
            }
        }
    }

    // ---- 覆盖集合由 JSON 声明构建（旧版按 r.merges 构建，是自证循环）----
    let mut covered: HashSet<(usize, usize)> = HashSet::new();
    for rect in &jm {
        for sr in rect.0..=rect.2 {
            for sc in rect.1..=rect.3 {
                if (sr, sc) != (rect.0, rect.1) {
                    covered.insert((sr, sc));
                }
            }
        }
    }
    covered
}
