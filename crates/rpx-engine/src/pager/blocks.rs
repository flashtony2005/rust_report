//! 禁止拆分的合并格构成连续块；超大块整块占一页，不在块内反复断页。
use crate::arena::CalcGrid;

pub(super) fn ends(grid: &CalcGrid, rows: bool) -> Vec<i32> {
    let len = if rows { grid.rows } else { grid.cols };
    let mut ends = (1..=len).collect::<Vec<_>>();
    let mut covered = std::collections::HashSet::new();
    let mut areas = grid.merged.iter().collect::<Vec<_>>();
    areas.sort_by_key(|m| (m.r, m.c));
    for m in areas {
        if covered.contains(&(m.r, m.c)) {
            continue;
        }
        for r in m.r..m.r + m.row_merge {
            for c in m.c..m.c + m.col_merge {
                covered.insert((r, c));
            }
        }
        let style = grid
            .get(m.r, m.c)
            .and_then(|c| grid.print_styles.get(&c.origin));
        if style.is_none_or(|s| s.split) {
            continue;
        }
        if !rows
            && grid
                .row_types
                .get((m.r - 1) as usize)
                .is_some_and(|t| [-93, -96, -92, -89].contains(t))
        {
            continue;
        }
        let (begin, span) = if rows {
            (m.r, m.row_merge)
        } else {
            (m.c, m.col_merge)
        };
        for i in begin..(begin + span).min(len + 1) {
            ends[(i - 1) as usize] = ends[(i - 1) as usize].max(begin + span - 1);
        }
    }
    // 重叠块传递合并，例如 1..3 与 3..5 变成 1..5。
    for i in (0..ends.len()).rev() {
        let end = ends[i].min(len) as usize;
        ends[i] = ends[i..end].iter().copied().max().unwrap_or(i as i32 + 1);
    }
    ends
}
