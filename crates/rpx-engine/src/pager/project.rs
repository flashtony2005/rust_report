//! 将全局网格投影到页内，保留重复区带并拆分跨页合并。
use super::PageInfo;
use crate::arena::{CalcCell, CalcGrid, MergedCell};
use std::collections::{HashMap, HashSet};

/// 生成独立页网格。行列映射是唯一数据来源，避免区间包含前页数据。
pub fn project_page(grid: &CalcGrid, page: &PageInfo) -> CalcGrid {
    let mut out = if page.panels.is_empty() {
        super::compose::horizontal(grid, page)
    } else {
        super::compose::columns(grid, page)
    };
    super::finishing::apply(&mut out);
    out
}

pub(super) fn project(grid: &CalcGrid, page: &PageInfo) -> CalcGrid {
    let rs = &page.row_indices;
    let cs = &page.col_indices;
    let mut out = CalcGrid::new(rs.len() as i32, cs.len() as i32);
    out.unit = grid.unit;
    out.print_setup = grid.print_setup.clone();
    out.print_options = grid.print_options.clone();
    out.print_styles = grid.print_styles.clone();
    out.page_background = grid.page_background.clone();
    for graphic in &grid.graphics {
        if let (Some(r), Some(c)) = (
            rs.iter().position(|r| *r == graphic.row),
            cs.iter().position(|c| *c == graphic.col),
        ) {
            let mut graphic = graphic.clone();
            graphic.row = r as i32 + 1;
            graphic.col = c as i32 + 1;
            out.graphics.push(graphic);
        }
    }
    let cells: HashMap<_, _> = grid.cells.iter().map(|c| ((c.row, c.col), c)).collect();
    for (r, &gr) in rs.iter().enumerate() {
        out.row_types.push(
            grid.row_types
                .get((gr - 1) as usize)
                .copied()
                .unwrap_or(-91),
        );
        out.row_heights.push(
            grid.row_heights
                .get((gr - 1) as usize)
                .copied()
                .unwrap_or(0.0),
        );
        out.row_visible.push(
            grid.row_visible
                .get((gr - 1) as usize)
                .copied()
                .unwrap_or(true),
        );
        for (c, &gc) in cs.iter().enumerate() {
            if let Some(cell) = cells.get(&(gr, gc)) {
                let mut cell = (*cell).clone();
                cell.row = r as i32 + 1;
                cell.col = c as i32 + 1;
                out.cells.push(cell);
            }
        }
    }
    for &c in cs {
        out.col_types
            .push(grid.col_types.get((c - 1) as usize).copied().unwrap_or(-79));
        out.col_widths.push(
            grid.col_widths
                .get((c - 1) as usize)
                .copied()
                .unwrap_or(0.0),
        );
        out.col_visible.push(
            grid.col_visible
                .get((c - 1) as usize)
                .copied()
                .unwrap_or(true),
        );
    }
    let mut taken = HashSet::new();
    let mut merges: Vec<_> = grid.merged.iter().collect();
    merges.sort_by_key(|m| (m.r, m.c));
    for m in merges {
        if taken.contains(&(m.r, m.c)) {
            continue;
        }
        for r in m.r..m.r + m.row_merge {
            for c in m.c..m.c + m.col_merge {
                taken.insert((r, c));
            }
        }
        for (r0, r1) in runs(rs, m.r, m.r + m.row_merge) {
            for (c0, c1) in runs(cs, m.c, m.c + m.col_merge) {
                let mut anchor = cells
                    .get(&(m.r, m.c))
                    .map(|c| (*c).clone())
                    .unwrap_or_else(|| CalcCell::bare(m.r, m.c, m.value.clone()));
                anchor.row = r0;
                anchor.col = c0;
                out.cells.retain(|c| c.row != r0 || c.col != c0);
                out.cells.push(anchor);
                if r0 == r1 && c0 == c1 {
                    continue;
                }
                for r in r0..=r1 {
                    for c in c0..=c1 {
                        out.merged.push(MergedCell::new(
                            1,
                            r,
                            c,
                            m.value.clone(),
                            r1 - r0 + 1,
                            c1 - c0 + 1,
                        ));
                    }
                }
            }
        }
    }
    out.cells.sort_by_key(|c| (c.row, c.col));
    out
}

fn runs(indices: &[i32], begin: i32, end: i32) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::new();
    let mut previous = None;
    for (i, &source) in indices.iter().enumerate() {
        if !(begin..end).contains(&source) {
            previous = None;
            continue;
        }
        let local = i as i32 + 1;
        if previous == Some(source - 1) {
            if let Some(last) = out.last_mut() {
                last.1 = local;
            }
        } else {
            out.push((local, local));
        }
        previous = Some(source);
    }
    out
}
