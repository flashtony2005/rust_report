//! 以几何边界的并集拼接分栏，避免不同栏的行高互相牵动。
use super::{project::project, unit_to_pt, usable_page_pt, PageInfo};
use crate::arena::{CalcGrid, MergedCell};
use std::collections::HashSet;

fn edges(sizes: &[f32], visible: &[bool]) -> Vec<f32> {
    let mut out = vec![0.0];
    for (i, &s) in sizes.iter().enumerate() {
        out.push(
            out.last().copied().unwrap_or(0.0)
                + if visible.get(i) == Some(&false) {
                    0.0
                } else {
                    s
                },
        )
    }
    out
}
fn total(g: &CalcGrid) -> (f32, f32) {
    (
        *edges(&g.col_widths, &g.col_visible).last().unwrap_or(&0.0),
        *edges(&g.row_heights, &g.row_visible).last().unwrap_or(&0.0),
    )
}
fn band(grid: &CalcGrid, p: &PageInfo, types: &[i8]) -> CalcGrid {
    let mut p = p.clone();
    p.panels.clear();
    p.row_indices
        .retain(|r| types.contains(&grid.row_types[(*r - 1) as usize]));
    // 全页区带使用完整宽度；分栏只重复表头。
    p.col_indices = (1..=grid.cols).collect();
    project(grid, &p)
}
fn stretch(g: &mut CalcGrid, width: f32) {
    let old = total(g).0;
    if old > 0.0 {
        let ratio = width / old;
        let mut flexible = g
            .cells
            .iter()
            .filter(|cell| {
                g.print_styles
                    .get(&cell.origin)
                    .is_some_and(|s| s.stretch_when_paged)
            })
            .map(|cell| (cell.col - 1) as usize)
            .collect::<Vec<_>>();
        flexible.sort_unstable();
        flexible.dedup();
        if flexible.is_empty() && !g.col_widths.is_empty() {
            flexible.push(g.col_widths.len() - 1)
        }
        let flexible_width = flexible.iter().map(|i| g.col_widths[*i]).sum::<f32>();
        let fixed = old - flexible_width;
        for (i, w) in g.col_widths.iter_mut().enumerate() {
            if fixed >= width || flexible_width <= 0.0 {
                *w *= ratio
            } else if flexible.contains(&i) {
                *w *= (width - fixed) / flexible_width
            }
        }
        for visual in &mut g.graphics {
            visual.width = (visual.width as f32 * ratio).round() as u32
        }
    }
}

/// 横向整幅重复与切分使用各自的列映射；标题合并格保留整幅。
pub(super) fn horizontal(grid: &CalcGrid, page: &PageInfo) -> CalcGrid {
    use super::options::Repeat;
    let full = |r: i32| match grid.row_types.get((r - 1) as usize) {
        Some(-93 | -92) => matches!(grid.print_options.header_x, Repeat::First | Repeat::Every),
        Some(-96 | -89) => {
            matches!(grid.print_options.title_x, Repeat::First | Repeat::Every)
                || grid
                    .merged
                    .iter()
                    .any(|m| m.r <= r && r < m.r + m.row_merge && m.col_merge > 1)
        }
        _ => false,
    };
    if !page.row_indices.iter().copied().any(full) || page.col_indices.len() == grid.cols as usize {
        return project(grid, page);
    }
    let width = page
        .col_indices
        .iter()
        .map(|c| {
            if grid.col_visible.get((c - 1) as usize) == Some(&false) {
                0.0
            } else {
                grid.col_widths
                    .get((c - 1) as usize)
                    .copied()
                    .unwrap_or(0.0)
            }
        })
        .sum();
    let mut nodes = Vec::new();
    let mut offset = 0.0;
    let mut start = 0;
    while start < page.row_indices.len() {
        let whole = full(page.row_indices[start]);
        let mut end = start + 1;
        while end < page.row_indices.len() && full(page.row_indices[end]) == whole {
            end += 1
        }
        let mut p = page.clone();
        p.panels.clear();
        p.row_indices = page.row_indices[start..end].to_vec();
        if whole {
            p.col_indices = (1..=grid.cols).collect()
        }
        let mut part = project(grid, &p);
        if whole {
            stretch(&mut part, width)
        }
        let h = total(&part).1;
        nodes.push((part, 0.0, offset));
        offset += h;
        start = end;
    }
    mosaic(grid, nodes, width)
}
pub(super) fn columns(grid: &CalcGrid, page: &PageInfo) -> CalcGrid {
    let n = grid.print_setup.table_column_num as f32;
    let factor = unit_to_pt(1.0, grid.unit) as f32;
    let gap = grid.print_options.column_gap * 72.0 / 25.4 / factor;
    let page_width = usable_page_pt(&grid.print_setup).0 as f32 / factor;
    let slot = (page_width - gap * (n - 1.0)) / n;
    let mut top = band(grid, &page.panels[0], &[-93, -96]);
    let mut bottom = band(
        grid,
        page.panels.last().unwrap_or(&page.panels[0]),
        &[-92, -89, -94],
    );
    stretch(&mut top, page_width);
    stretch(&mut bottom, page_width);
    let top_h = total(&top).1;
    let mut nodes = Vec::new();
    let mut body_h = 0.0_f32;
    for (i, p) in page.panels.iter().enumerate() {
        let mut p = p.clone();
        p.row_indices
            .retain(|r| ![-93, -96, -92, -89, -94].contains(&grid.row_types[(*r - 1) as usize]));
        let body = horizontal(grid, &p);
        body_h = body_h.max(total(&body).1);
        nodes.push((body, i as f32 * (slot + gap), top_h));
    }
    nodes.push((top, 0.0, 0.0));
    nodes.push((bottom, 0.0, top_h + body_h));
    mosaic(grid, nodes, page_width)
}
fn unique(values: &mut Vec<f32>) {
    values.sort_by(f32::total_cmp);
    values.dedup_by(|a, b| (*a - *b).abs() < 0.0001)
}
fn index(edges: &[f32], n: f32) -> i32 {
    edges
        .iter()
        .position(|e| (*e - n).abs() < 0.0001)
        .unwrap_or(0) as i32
        + 1
}
fn mosaic(original: &CalcGrid, nodes: Vec<(CalcGrid, f32, f32)>, width: f32) -> CalcGrid {
    let mut xs = vec![0.0, width];
    let mut ys = vec![0.0];
    for (g, x, y) in &nodes {
        xs.extend(edges(&g.col_widths, &g.col_visible).iter().map(|v| v + x));
        ys.extend(edges(&g.row_heights, &g.row_visible).iter().map(|v| v + y))
    }
    unique(&mut xs);
    unique(&mut ys);
    let mut out = CalcGrid::new(
        ys.len().saturating_sub(1) as i32,
        xs.len().saturating_sub(1) as i32,
    );
    out.unit = original.unit;
    out.print_setup = original.print_setup.clone();
    out.print_options = original.print_options.clone();
    out.page_background = original.page_background.clone();
    out.print_styles = original.print_styles.clone();
    out.row_heights = ys.windows(2).map(|w| w[1] - w[0]).collect();
    out.col_widths = xs.windows(2).map(|w| w[1] - w[0]).collect();
    out.row_visible = vec![true; out.rows as usize];
    out.col_visible = vec![true; out.cols as usize];
    out.row_types = vec![-91; out.rows as usize];
    for (g, x, y) in nodes {
        let xedges = edges(&g.col_widths, &g.col_visible);
        let yedges = edges(&g.row_heights, &g.row_visible);
        let mut covered = HashSet::new();
        let mut areas = Vec::new();
        let mut sorted = g.merged.iter().collect::<Vec<_>>();
        sorted.sort_by_key(|m| (m.r, m.c));
        for m in sorted {
            if covered.contains(&(m.r, m.c)) {
                continue;
            }
            for r in m.r..m.r + m.row_merge {
                for c in m.c..m.c + m.col_merge {
                    covered.insert((r, c));
                }
            }
            areas.push(m)
        }
        for cell in &g.cells {
            let area = areas.iter().find(|m| m.r == cell.row && m.c == cell.col);
            if covered.contains(&(cell.row, cell.col)) && area.is_none() {
                continue;
            }
            let (rm, cm) = area.map(|m| (m.row_merge, m.col_merge)).unwrap_or((1, 1));
            let (r, c) = ((cell.row - 1) as usize, (cell.col - 1) as usize);
            let (r0, r1) = (
                index(&ys, yedges[r] + y),
                index(&ys, yedges[r + rm as usize] + y) - 1,
            );
            let (c0, c1) = (
                index(&xs, xedges[c] + x),
                index(&xs, xedges[c + cm as usize] + x) - 1,
            );
            if r1 < r0 || c1 < c0 {
                continue;
            }
            let mut cell = cell.clone();
            cell.row = r0;
            cell.col = c0;
            out.cells.push(cell.clone());
            for rr in r0..=r1 {
                out.row_types[(rr - 1) as usize] = g.row_types.get(r).copied().unwrap_or(-91);
            }
            if r1 > r0 || c1 > c0 {
                for rr in r0..=r1 {
                    for cc in c0..=c1 {
                        out.merged.push(MergedCell::new(
                            1,
                            rr,
                            cc,
                            cell.value.clone(),
                            r1 - r0 + 1,
                            c1 - c0 + 1,
                        ))
                    }
                }
            }
        }
        for mut v in g.graphics {
            v.row = index(&ys, yedges[(v.row - 1) as usize] + y);
            v.col = index(&xs, xedges[(v.col - 1) as usize] + x);
            out.graphics.push(v)
        }
    }
    out.cells.sort_by_key(|c| (c.row, c.col));
    out
}
