//! 模板打印布局。每个页面/栏持有明确的行列清单。
use super::{arith::fits, unit_to_pt, PageInfo, Pagination};
use crate::{
    arena::CalcGrid,
    error::{EngineError, Result},
};
use rpx_core::print_setup::PrintSetup;

fn height(g: &CalcGrid, r: i32) -> f64 {
    if g.row_visible.get((r - 1) as usize) == Some(&false) {
        return 0.0;
    }
    unit_to_pt(
        g.row_heights.get((r - 1) as usize).copied().unwrap_or(0.0),
        g.unit,
    )
}
fn width(g: &CalcGrid, c: i32) -> f64 {
    if g.col_visible.get((c - 1) as usize) == Some(&false) {
        return 0.0;
    }
    unit_to_pt(
        g.col_widths.get((c - 1) as usize).copied().unwrap_or(0.0),
        g.unit,
    )
}
struct Flow {
    rows: Vec<i32>,
    next_page: bool,
}
fn rows(g: &CalcGrid, ps: &PrintSetup, limit: f64) -> Vec<Flow> {
    if ps.pager_style == 0 || !g.print_options.vertical_split {
        return vec![Flow {
            rows: (1..=g.rows).collect(),
            next_page: false,
        }];
    }
    let band = |types: &[i8]| {
        (1..=g.rows)
            .filter(|r| types.contains(&g.row_types.get((r - 1) as usize).copied().unwrap_or(-91)))
            .collect::<Vec<_>>()
    };
    let o = &g.print_options;
    let headers = |sheet: usize| {
        band(&[-93, -96, -95])
            .into_iter()
            .filter(|r| match g.row_types[(*r - 1) as usize] {
                -93 => o.header_y.included(sheet),
                -96 => o.title_y.included(sheet),
                _ => o.row_header_every || sheet == 0,
            })
            .collect::<Vec<_>>()
    };
    let data = band(&[-91, -90]);
    let blocks = super::blocks::ends(g, true);
    let mut flows = Vec::new();
    let mut groups = Vec::<i32>::new();
    let (mut index, mut sheet, mut column) = (0, 0, 0);
    loop {
        let mut page = headers(sheet);
        if column == 0 || ps.column_repeated_group_header {
            page.extend(groups.iter().copied())
        }
        let footers = if o.header_y.included(sheet) {
            band(&[-92])
        } else {
            Vec::new()
        };
        let repeat_tail = if o.row_footer_every {
            band(&[-94])
        } else {
            Vec::new()
        };
        let mut used = page
            .iter()
            .chain(&footers)
            .chain(&repeat_tail)
            .map(|r| height(g, *r))
            .sum::<f64>();
        let mut count = 0;
        let mut locked_until = 0;
        let mut next_page = false;
        while let Some(&r) = data.get(index) {
            let prev = index.checked_sub(1).map(|i| (data[i] - 1) as usize);
            let forced_page =
                ps.pager_style == 2 && prev.is_some_and(|i| g.row_break_page.get(i) == Some(&true));
            let forced_column = ps.pager_style == 2
                && prev.is_some_and(|i| g.row_break_column.get(i) == Some(&true));
            let end = blocks.get((r - 1) as usize).copied().unwrap_or(r);
            let block = data[index..]
                .iter()
                .take_while(|rr| **rr <= end)
                .copied()
                .collect::<Vec<_>>();
            let full = if ps.pager_style == 1 {
                count + block.len() > ps.row_num_per_page as usize
            } else {
                !fits(block.iter().map(|r| height(g, *r)).sum(), used, limit)
            };
            if count > 0 && r > locked_until && (forced_page || forced_column || full) {
                next_page = forced_page;
                break;
            }
            // 不在新栏重复紧接着就要读到的同一个组头。
            if count == 0 && page.last() == Some(&r) {
                page.pop();
                used -= height(g, r)
            }
            page.push(r);
            locked_until = locked_until.max(end);
            used += height(g, r);
            count += 1;
            index += 1;
            if g.row_types.get((r - 1) as usize) == Some(&-90) {
                let level = g
                    .row_group_levels
                    .get((r - 1) as usize)
                    .copied()
                    .unwrap_or(1);
                groups.retain(|r| {
                    g.row_group_levels
                        .get((r - 1) as usize)
                        .copied()
                        .unwrap_or(1)
                        < level
                });
                groups.push(r);
            }
        }
        let finished = index == data.len();
        if finished {
            let tail = band(&[-89, -94]);
            let extra = tail
                .iter()
                .filter(|r| !repeat_tail.contains(r))
                .map(|r| height(g, *r))
                .sum::<f64>();
            if !tail.is_empty() && ps.pager_style == 2 && count > 0 && !fits(extra, used, limit) {
                page.extend(&repeat_tail);
                page.extend(&footers);
                flows.push(Flow {
                    rows: page,
                    next_page: false,
                });
                page = headers(sheet + 1);
            }
            page.extend(tail);
        } else {
            page.extend(repeat_tail)
        }
        page.extend(footers);
        flows.push(Flow {
            rows: page,
            next_page,
        });
        if finished {
            break;
        }
        column += 1;
        if next_page || column >= ps.table_column_num as usize {
            column = 0;
            sheet += 1
        }
    }
    flows
}
fn columns(g: &CalcGrid, ps: &PrintSetup, limit: f64) -> Vec<Vec<i32>> {
    if ps.pager_style == 0 || !g.print_options.horizontal_split {
        return vec![(1..=g.cols).collect()];
    }
    let band = |kind| {
        (1..=g.cols)
            .filter(|c| g.col_types.get((c - 1) as usize).copied().unwrap_or(-79) == kind)
            .collect::<Vec<_>>()
    };
    let (left, right, data) = (band(-80), band(-78), band(-79));
    let blocks = super::blocks::ends(g, false);
    let mut pages = Vec::new();
    let mut index = 0;
    loop {
        let ls = if g.print_options.column_header_every || pages.is_empty() {
            left.clone()
        } else {
            Vec::new()
        };
        let rs = if g.print_options.column_footer_every || pages.is_empty() {
            right.clone()
        } else {
            Vec::new()
        };
        let mut used = ls.iter().chain(&rs).map(|c| width(g, *c)).sum::<f64>();
        let mut page = ls;
        let mut count = 0;
        let mut locked_until = 0;
        while let Some(&c) = data.get(index) {
            let forced = ps.pager_style == 2
                && index > 0
                && g.col_break_page.get((data[index - 1] - 1) as usize) == Some(&true);
            let end = blocks.get((c - 1) as usize).copied().unwrap_or(c);
            let cost = data[index..]
                .iter()
                .take_while(|cc| **cc <= end)
                .map(|c| width(g, *c))
                .sum();
            if count > 0 && c > locked_until && (forced || !fits(cost, used, limit)) {
                break;
            }
            page.push(c);
            locked_until = locked_until.max(end);
            used += width(g, c);
            count += 1;
            index += 1;
        }
        page.extend(rs);
        pages.push(page);
        if index == data.len() {
            break;
        }
    }
    pages
}
fn page(g: &CalcGrid, rs: Vec<i32>, cs: Vec<i32>, x: usize) -> PageInfo {
    let o = &g.print_options;
    let rs = rs
        .into_iter()
        .filter(|r| match g.row_types.get((r - 1) as usize) {
            Some(-93 | -92) => o.header_x.included(x),
            Some(-96 | -89) => o.title_x.included(x),
            _ => true,
        })
        .collect::<Vec<_>>();
    let values = cs
        .first()
        .map(|&c| {
            rs.iter()
                .filter_map(|&r| g.get(r, c))
                .filter(|c| !c.value.is_null())
                .map(|c| c.value.clone())
                .collect()
        })
        .unwrap_or_default();
    PageInfo {
        panels: Vec::new(),
        page: 0,
        rows: rs.len() as i32,
        row_range: range(&rs),
        col_range: range(&cs),
        row_indices: rs,
        col_indices: cs,
        col1_values: values,
    }
}
pub(super) fn paginate(g: &CalcGrid, ps: &PrintSetup, w: f64, h: f64) -> Result<Pagination> {
    let n = if ps.pager_style == 0 {
        1
    } else {
        ps.table_column_num as usize
    };
    if n > 1 && g.row_visible.contains(&false) {
        return Err(EngineError::UnsupportedPaging {
            detail: "多栏打印不允许隐藏行".into(),
        });
    }
    let column_w =
        (w - f64::from(g.print_options.column_gap) * 72.0 / 25.4 * (n - 1) as f64) / n as f64;
    if column_w <= 0.0 {
        return Err(EngineError::UnsupportedPaging {
            detail: "栏距占满了纸张宽度".into(),
        });
    }
    let ys = rows(g, ps, h);
    let xs = columns(g, ps, column_w);
    let mut pages = Vec::new();
    let mut start = 0;
    let mut y_pages = 0;
    while start < ys.len() {
        let mut end = start + 1;
        while end < ys.len() && end - start < n && !ys[end - 1].next_page {
            end += 1
        }
        for (x, cols) in xs.iter().enumerate() {
            let mut p = page(g, ys[start].rows.clone(), cols.clone(), x);
            if n > 1 {
                p.panels = ys[start..end]
                    .iter()
                    .map(|s| page(g, s.rows.clone(), cols.clone(), x))
                    .collect()
            }
            p.page = pages.len() as i32 + 1;
            pages.push(p);
        }
        start = end;
        y_pages += 1;
    }
    Ok(Pagination {
        paper_w: w as f32,
        paper_h: h as f32,
        columns: n as i32,
        page_count: pages.len() as i32,
        x_pages: xs.len() as i32,
        y_pages,
        row_num_per_page: ps.row_num_per_page,
        pages,
    })
}
fn range(indices: &[i32]) -> (i32, i32) {
    (
        indices.iter().min().copied().unwrap_or(0),
        indices.iter().max().copied().unwrap_or(0),
    )
}
