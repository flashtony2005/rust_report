//! 分页的**算法半场**（2026-10-14 拆自 `pager.rs`，G4 行数门禁）：
//! 纵向断页（[`split_rows`]）、横向断页（[`split_cols`]）、页首/页脚区带与
//! 分组头副本的扫描（[`leading_run`] / [`trailing_run`] / [`data_rows`]）。
//!
//! 这些函数**不关心页尺寸从哪来**（探针常量 or 模板真值，见 [`super::paper`]）——
//! 调用方传 `page_h` / `page_w`（pt）进来。Java 事实源与逐条边界仍在
//! [`super`] 模块文档（`SimplePager._$5` / `_$1` / `_$2` 的行号引用），此处不复制。

use super::arith::fits;
use crate::arena::CalcGrid;
use crate::value::Value;

/// `IRowCell.TYPE_GROUP_HEADER`：分组头行。
pub const ROW_TYPE_GROUP_HEADER: i8 = -90;

/// 一页消耗的行。`kind` 决定它是**数据区行**（按数据序各出现一次）、
/// **页首区带行**（每页都在页顶出现），还是**分组头副本**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRow {
    /// 网格行号（1 起）—— 副本记的是它复制自的那一行。
    pub row: i32,
    /// 这一行在页内的角色。
    pub kind: PageRowKind,
}

/// [`PageRow`] 的角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRowKind {
    /// 数据区（`_$18`）内的行，按数据序各出现**一次**。
    Data,
    /// 页首区带行（`-93` 页眉 / `-96` 标题 / `-95` 表头），**每页**都在页顶出现
    /// （`SimplePager._$5:327-349`）。它的高度在页首**一次性**加进 `_$26`。
    Header,
    /// 分组头副本（`copyRowTo(..., needclone = true)`）—— 吃页高、也计入该页 `rows`。
    GroupCopy,
}

/// 页首区带的行类型，**按页内出现顺序**：页眉（`_$24 = _$4(r, -93)`）→
/// 标题（`_$22 = _$4(r, -96)`）→ 表头（`_$20 = _$4(r, -95)`）。
pub(crate) const PAGE_PREFIX_TYPES: [i8; 3] = [-93, -96, -95];

/// 页脚区带的类型：页脚（`_$23 = _$3(r, -92)`）、标题脚（`_$21 = _$3(r, -89)`）、
/// 表脚（`_$19 = _$3(r, -94)`）。
///
/// ⛔ 只加**高度**、不把行复制到页顶（`_$5:329/334/339`）。夹具里三类都不存在
/// ⇒ 高度恒 0；本 crate 仍按同式累加，以免将来有夹具时静默少算。
pub(crate) const PAGE_FOOTER_TYPES: [i8; 3] = [-92, -89, -94];

/// 数据区行类型（`SimplePager._$2(r):1318`）：`-91`（普通）与 `-90`（分组头）。
pub(crate) fn is_data_row_type(t: i8) -> bool {
    t == -91 || t == -90
}

/// `_$4(r, rowType)`（`SimplePager.java:1391-1412`）：类型 `t` 的**首个连续段**。
///
/// 扫描从第 1 行起；遇到 `t` 记下起点并继续，一旦在起点之后遇到非 `t` 就**跳出**
/// —— 所以它是"第一个连续段"，不是"所有该类型的行"。
pub fn leading_run(types: &[i8], t: i8) -> Option<(i32, i32)> {
    let mut start: Option<i32> = None;
    let mut end = 0i32;
    for (i, &ty) in types.iter().enumerate() {
        let row = i as i32 + 1;
        if ty == t {
            if start.is_none() {
                start = Some(row);
            }
            end = row;
        } else if start.is_some() {
            break;
        }
    }
    start.map(|s| (s, end))
}

/// `_$3(r, rowType)`（`SimplePager.java:1414-1435`）：类型 `t` 的**末个连续段**。
pub fn trailing_run(types: &[i8], t: i8) -> Option<(i32, i32)> {
    let mut end: Option<i32> = None;
    let mut start = 0i32;
    for (i, &ty) in types.iter().enumerate().rev() {
        let row = i as i32 + 1;
        if ty == t {
            if end.is_none() {
                end = Some(row);
            }
            start = row;
        } else if end.is_some() {
            break;
        }
    }
    end.map(|e| (start, e))
}

/// `_$2(r)`（`SimplePager.java:1304-1346`）的**行**范围：首个到末个 `-91`/`-90` 行。
///
/// ⛔ 这个范围**可以包含非数据行**：`engine_head_left` 的数据区是行 2..11，其中
/// 行 8~10 是标题行（`-96`）—— 它们既被当作标题复制到页顶，**也**在数据循环里
/// 作为数据行出现一次（夹具的 `col1_values` 里 `100,200,150` 出现两遍即此）。
/// 本 crate 的列范围固定取 `1..=cols`：夹具的 `colType` 全是 `-79`
/// （数据列），故 `_$2(r)` 的列范围与全列一致；列区带未建模（守卫
/// [`super::guard::ensure_data_columns`] 会显式拦住这类模板）。
pub fn data_rows(types: &[i8]) -> Option<(i32, i32)> {
    let begin = types.iter().position(|&t| is_data_row_type(t))? as i32 + 1;
    let end = types.iter().rposition(|&t| is_data_row_type(t))? as i32 + 1;
    Some((begin, end))
}

/// 纵向断页结果（与横向页组合前的中间态）。
#[derive(Debug, Clone)]
pub struct YSplit {
    /// 每页的行清单（含页首区带与分组头副本），顺序即页内行序。
    pub pages: Vec<Vec<PageRow>>,
}

/// 纵向断页 + 页首区带 + 分组头每页重复
/// （`SimplePager._$5` 的 `pagerStyle == 2 && columns == 1` 分支）。
///
/// # 页的构成（顺序即契约，`_$5:327-349` + `:352-410`）
///
/// ```text
/// [页眉 -93][标题 -96][表头 -95]   ← 每页都有；高度在页首一次性累加
/// [分组头副本 × N]                 ← 每页都有；高度逐行累加、行数计入
/// [数据区行 ...]                   ← 按数据序各出现一次；逐行判放不下
/// ```
///
/// # 分组头规则
///
/// 产品在每页开头把「当前所在分组的父链」复制到页顶（`_$5:352-387`），副本也吃页高。
/// 本实现按 `-90` 行维护一个**打开的分组头栈**：遇到 `-90` 行就把它入栈，
/// 翻页时把整个栈按顺序复制到页顶。单层分组头（`engine_pager`）与产品的父链**完全一致**；
/// 多层时本实现按"所有未闭合的分组头"复制，与产品的"严格更小层级链"在
/// **同级连续出现**的形态下等价（夹具无多层样本，见模块文档的边界清单）。
pub fn split_rows(grid: &CalcGrid, page_h: f64) -> YSplit {
    let types = &grid.row_types;
    let height_of = |r: i32| -> f64 {
        if r < 1 {
            return 0.0;
        }
        let index = (r - 1) as usize;
        if grid.row_visible.get(index) == Some(&false) {
            return 0.0;
        }
        grid.row_heights
            .get(index)
            .map(|h| super::arith::unit_to_pt(*h, grid.unit))
            .unwrap_or(0.0)
    };

    // 页首区带（行号，按页内顺序）与它的总高。
    let mut prefix: Vec<i32> = Vec::new();
    for t in PAGE_PREFIX_TYPES {
        if let Some((a, b)) = leading_run(types, t) {
            prefix.extend(a..=b);
        }
    }
    let prefix_h: f64 = prefix.iter().map(|&r| height_of(r)).sum();
    // 页脚区带只加高，不复制行。
    let footer_h: f64 = PAGE_FOOTER_TYPES
        .iter()
        .filter_map(|&t| trailing_run(types, t))
        .map(|(a, b)| (a..=b).map(height_of).sum::<f64>())
        .sum();

    let (data_begin, data_end) = data_rows(types).unwrap_or((1, 0));

    let mut pages: Vec<Vec<PageRow>> = Vec::new();
    // 打开的分组头栈（行号，按遇到顺序）。
    let mut open_groups: Vec<i32> = Vec::new();
    let mut next = data_begin;
    while next <= data_end {
        let mut page: Vec<PageRow> = Vec::new();
        let mut used = prefix_h + footer_h;
        for &r in &prefix {
            page.push(PageRow {
                row: r,
                kind: PageRowKind::Header,
            });
        }
        for &g in &open_groups {
            page.push(PageRow {
                row: g,
                kind: PageRowKind::GroupCopy,
            });
            used += height_of(g);
        }
        let mut data_on_page = 0usize;
        while next <= data_end {
            let cost = height_of(next);
            // 当前可见行若紧跟在 breakPage 行后，先断页（`_$1(row):1516`）。
            let visible = grid
                .row_visible
                .get((next - 1) as usize)
                .copied()
                .unwrap_or(true);
            let forced_break = visible
                && next > data_begin
                && grid
                    .row_break_page
                    .get((next - 2) as usize)
                    .copied()
                    .unwrap_or(false);
            if data_on_page > 0 && forced_break {
                break;
            }
            // 页上已有数据行且放不下 ⇒ 断页（`candisplayrow == 2`）。
            // ⛔ 页上还没有数据行时**必须放下**：产品此时走拆分分支，而
            // `if (row != firstDataRow && …) break` 对首页第一行不成立（`_$5:424`）。
            if data_on_page > 0 && !fits(cost, used, page_h) {
                break;
            }
            page.push(PageRow {
                row: next,
                kind: PageRowKind::Data,
            });
            used += cost;
            data_on_page += 1;
            if types.get((next - 1) as usize).copied() == Some(ROW_TYPE_GROUP_HEADER) {
                open_groups.push(next);
            }
            next += 1;
        }
        if data_on_page == 0 {
            // 防御：数据区为空却仍在循环（不可能，但别死循环）。
            break;
        }
        pages.push(page);
    }
    if pages.is_empty() {
        // 无数据区：仍产出一页（页首区带照旧），与产品"至少一页"一致。
        let page: Vec<PageRow> = prefix
            .iter()
            .map(|&r| PageRow {
                row: r,
                kind: PageRowKind::Header,
            })
            .collect();
        pages.push(page);
    }
    YSplit { pages }
}

/// 横向断页（`SimplePager._$2(currPageRows, unit):1836-1953` 的贪心累加）。
///
/// 返回每页的 `(首列, 末列)`。若全部列宽之和 ≤ 可用宽且无 `breakPage` 列，
/// 产品走单列页短路（`cols = [-1]`，`:1840-1852`）—— 本实现等价地返回一个 `(1, cols)`。
pub fn split_cols(grid: &CalcGrid, page_w: f64) -> Vec<(i32, i32)> {
    let cols = grid.cols.max(0) as usize;
    let width_of = |c: usize| -> f64 {
        let index = c.wrapping_sub(1);
        if grid.col_visible.get(index) == Some(&false) {
            return 0.0;
        }
        grid.col_widths
            .get(index)
            .map(|w| super::arith::unit_to_pt(*w, grid.unit))
            .unwrap_or(0.0)
    };
    let mut out: Vec<(i32, i32)> = Vec::new();
    if cols == 0 {
        return vec![(1, 0)];
    }
    let mut start = 1usize;
    let mut used = 0.0;
    for c in 1..=cols {
        let cost = width_of(c);
        let current_visible = grid.col_visible.get(c - 1).copied().unwrap_or(true);
        let manual_break = c > start
            && current_visible
            && grid.col_break_page.get(c - 2).copied().unwrap_or(false);
        // 手动断列按当前列的前一列 breakPage 处理；宽度断列仍使用千分位容差。
        if manual_break {
            out.push((start as i32, (c - 1) as i32));
            start = c;
            used = 0.0;
        }
        // 每页的**第一列**必定落在该页（产品此时抛 widthOutofPage，不另起一页）。
        if c > start && !fits(cost, used, page_w) {
            out.push((start as i32, (c - 1) as i32));
            start = c;
            used = 0.0;
        }
        used += cost;
    }
    out.push((start as i32, cols as i32));
    out
}

/// 该页**自己第 1 列**上的非空值序列（`CalcJson.java:153-164` 的扫描）。
///
/// ⛔ 按**页内行清单**取，不是按整张网格取 —— 否则分组头副本的值会漏掉、
/// 翻页后的首列也会取错（`engine_pager` 第 2~4 页的 `first_value` 全是副本的
/// `"group: R*"`）。
pub fn col_values(grid: &CalcGrid, page_rows: &[PageRow], base_col: i32) -> Vec<Value> {
    let mut out = Vec::new();
    if base_col < 1 || base_col > grid.cols {
        return out;
    }
    for pr in page_rows {
        if let Some(cell) = grid.get(pr.row, base_col) {
            if !cell.value.is_null() {
                out.push(cell.value.clone());
            }
        }
    }
    out
}
