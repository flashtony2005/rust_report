//! 页内呈现规则只改页副本，不改公式所见的原始数据。
use super::{options::EndSpace, unit_to_pt, usable_page_pt};
use crate::{
    arena::{CalcCell, CalcGrid},
    graphics::number,
    value::Value,
};

pub(super) fn apply(g: &mut CalcGrid) {
    let o = &g.print_options;
    for cell in &mut g.cells {
        if o.zero_as_blank && number(&cell.value) == Some(0.0) {
            cell.disp_value = Some(String::new())
        }
        if cell.value.is_null() && cell.disp_value.is_none() && !o.empty_value.is_empty() {
            cell.disp_value = Some(o.empty_value.clone())
        }
    }
    if o.output_empty || !o.empty_value.is_empty() {
        let occupied = g
            .cells
            .iter()
            .map(|c| (c.row, c.col))
            .chain(g.merged.iter().map(|m| (m.r, m.c)))
            .collect::<std::collections::HashSet<_>>();
        for r in 1..=g.rows {
            for c in 1..=g.cols {
                if !occupied.contains(&(r, c)) {
                    let mut cell = CalcCell::bare(r, c, Value::Null);
                    cell.disp_value = Some(o.empty_value.clone());
                    g.cells.push(cell)
                }
            }
        }
    }
    if g.print_setup.pager_style != 2 {
        return;
    }
    let factor = unit_to_pt(1.0, g.unit) as f32;
    let available = usable_page_pt(&g.print_setup).1 as f32 / factor;
    let used = g
        .row_heights
        .iter()
        .enumerate()
        .filter(|(i, _)| g.row_visible.get(*i) != Some(&false))
        .map(|(_, h)| h)
        .sum::<f32>();
    let extra = available - used;
    if extra <= 0.0 {
        return;
    }
    let data = (0..g.rows as usize)
        .filter(|i| g.row_types.get(*i) == Some(&-91) && g.row_visible.get(*i) != Some(&false))
        .collect::<Vec<_>>();
    if g.print_setup.blank_line_filled {
        let size = data
            .last()
            .and_then(|i| g.row_heights.get(*i))
            .copied()
            .filter(|h| *h > 0.0)
            .unwrap_or(20.0 / factor);
        let count = (extra / size).floor() as usize;
        let before = g
            .row_types
            .iter()
            .position(|t| [-92, -89, -94].contains(t))
            .unwrap_or(g.rows as usize);
        for cell in &mut g.cells {
            if cell.row > before as i32 {
                cell.row += count as i32
            }
        }
        for m in &mut g.merged {
            if m.r > before as i32 {
                m.r += count as i32
            }
        }
        for v in &mut g.graphics {
            if v.row > before as i32 {
                v.row += count as i32
            }
        }
        g.row_heights
            .splice(before..before, std::iter::repeat_n(size, count));
        g.row_types
            .splice(before..before, std::iter::repeat_n(-91, count));
        g.row_visible
            .splice(before..before, std::iter::repeat_n(true, count));
        g.rows += count as i32;
    } else {
        match o.end_space {
            EndSpace::None => {}
            EndSpace::Last => {
                if let Some(i) = data.last() {
                    g.row_heights[*i] += extra
                }
            }
            EndSpace::Average => {
                let n = data.len() as f32;
                for i in data {
                    g.row_heights[i] += extra / n
                }
            }
            EndSpace::Proportional => {
                let h = data.iter().map(|i| g.row_heights[*i]).sum::<f32>();
                if h > 0.0 {
                    for i in data {
                        g.row_heights[i] *= 1.0 + extra / h
                    }
                }
            }
        }
    }
}
