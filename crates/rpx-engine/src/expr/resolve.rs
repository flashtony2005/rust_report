//! `CSVariable` 的相对引用解析（M2-06）—— `CSVariable.java` 全文移植。
//!
//! 算法正文：本文件即正文（逐行带 `file:line`）；产品原文件
//! `reportlite/03-model/com/raqsoft/reportl/model/expression/CSVariable.java`。
//!
//! # 解析流水线（`calculate():105-121` + `getCell():236-239`）
//!
//! 1. `_$9` = `cs.getSource(id)`（**引擎矩阵**里的格；`source_by_id`）；
//! 2. `current` = `isSaved ? cs.getSavedCurrent() : cs.getCurrent()`
//!    （`$` 前缀的唯一效果，附件 G.1；两指针定义在 `ExtCellSet.java:578-596`）；
//! 3. `leftSource = _$9.getCommonLeftHead(current.getSource())`（`:373-382`：
//!    从 `_$9` 沿左链走，第一个 `isLeftHeadOf(current 的出处)` 的节点）；
//! 4. `_$8 = current.getCellLeftHead(leftSource)`（`:403-409`：从 current 沿链走，
//!    找**出处**等于 leftSource 的节点）；top 同理；
//! 5. `getCell() = _$4(_$8, _$7)` → `_$1(left, top)` 递归（`:168-234`）。
//!
//! # `_$1` 递归的分支结构（`CSVariable.java:168-234`，逐行对照）
//!
//! - `left`/`top` 为空 → 取 `cell00`；
//! - `source_of(left) == src` → **返回 left**；`source_of(top) == src` → 返回 top
//!   （"引用格就是 current 所挂链的锚" ⇒ 直接用 current 侧的实例）；
//! - 左链锚是 src 的 head（且不是直接 head）→ 沿 left 的左子格列表递归；
//! - 上链锚对称；
//! - `isDirectLeftHeadOf`（head==sub 的直接 head **且**在子格列表里，
//!   `ExtExtendCell.java:241-244`）→ `_$3`：扫 left 的左子格里出处==src 的实例，
//!   命中 `top.isTopHeadOf(cell)` 者返回；
//! - `left/top 出处都是 cell00` → 按 src 的 extensible 选 `_$3`(18) 或 `_$2`(19)。
//!
//! ⛔ 依赖的格能力全部来自 [`EvalCtx`]（本 crate 内的 trait，见 `eval.rs`）：
//! `source_of`（实例的引擎出处，`ExtNormalCell.getSource():139-141`）、
//! `ext_cells`（`_$7` 实例数组）、左右子格列表、head 链。

use crate::arena::CellId;
use crate::expr::eval::EvalCtx;

/// `getCommonLeftHead(other)`（`ExtNormalCell.java:409-419`）：
/// 从 `a` 沿左链走，第一个 `isLeftHeadOf(head, b)` 的节点；链尽 → `NONE`。
fn common_left_head(ctx: &dyn EvalCtx, a: CellId, b: CellId) -> CellId {
    let mut head = a;
    while head.is_some() {
        if is_left_head_of(ctx, head, b) {
            return head;
        }
        head = ctx.left_head(head);
    }
    CellId::NONE
}

/// `getCommonTopHead`（`:421-431`）：对称。
fn common_top_head(ctx: &dyn EvalCtx, a: CellId, b: CellId) -> CellId {
    let mut head = a;
    while head.is_some() {
        if is_top_head_of(ctx, head, b) {
            return head;
        }
        head = ctx.top_head(head);
    }
    CellId::NONE
}

/// `isLeftHeadOf`（`:391-398`）：自含——从 `sub` 自身起步沿链上溯。
pub(crate) fn is_left_head_of(ctx: &dyn EvalCtx, anchor: CellId, sub: CellId) -> bool {
    let mut head = sub;
    for _ in 0..4096 {
        if head.is_none() {
            return false;
        }
        if head == anchor {
            return true;
        }
        head = ctx.left_head(head);
    }
    false
}

/// `isTopHeadOf`（`:400-407`）：对称。
pub(crate) fn is_top_head_of(ctx: &dyn EvalCtx, anchor: CellId, sub: CellId) -> bool {
    let mut head = sub;
    for _ in 0..4096 {
        if head.is_none() {
            return false;
        }
        if head == anchor {
            return true;
        }
        head = ctx.top_head(head);
    }
    false
}

/// `getCellLeftHead(ExtCell source)`（`:403-409`）：从 `from` 沿链走，找
/// **出处**等于 `target` 的节点；链尽 → `NONE`。
fn walk_to_source(ctx: &dyn EvalCtx, from: CellId, target: CellId, top: bool) -> CellId {
    let mut head = from;
    for _ in 0..4096 {
        if head.is_none() {
            return CellId::NONE;
        }
        if ctx.source_of(head) == target {
            return head;
        }
        head = if top {
            ctx.top_head(head)
        } else {
            ctx.left_head(head)
        };
    }
    CellId::NONE
}

/// `isDirectLeftHeadOf`（`ExtExtendCell.java:241-244`）：
/// `sub.getCellLeftHead() == head && head._$6.contains(sub)`。
fn is_direct_left_head(ctx: &dyn EvalCtx, head: CellId, sub: CellId) -> bool {
    ctx.left_head(sub) == head && ctx.left_subs(head).contains(&sub)
}

/// `isDirectTopHeadOf`（`:246-249`）：对称。
fn is_direct_top_head(ctx: &dyn EvalCtx, head: CellId, sub: CellId) -> bool {
    ctx.top_head(sub) == head && ctx.top_subs(head).contains(&sub)
}

/// `calculate()` + `getCell()` 的合流：解析格号 `id` 在当前上下文里的**实例**。
/// `saved` = 是否带 `$`（用 `getSavedCurrent()`）。解析不到返回 `None`
/// （Java 返回 null，调用方按空值处理）。
pub fn resolve_cell(ctx: &dyn EvalCtx, id: &str, saved: bool) -> Option<CellId> {
    let src = ctx.source_by_id(id)?;
    let current = if saved {
        ctx.saved_current()
    } else {
        ctx.current()
    };
    if current.is_none() {
        return None;
    }
    let cur_src = ctx.source_of(current);
    let left_source = common_left_head(ctx, src, cur_src);
    let top_source = common_top_head(ctx, src, cur_src);
    let left = walk_to_source(ctx, current, left_source, false);
    let top = walk_to_source(ctx, current, top_source, true);
    resolve_impl(ctx, src, left, top, 0)
}

/// `_$4(left, top)` → `_$1(left, top)`（`CSVariable.java:168-234`）。
#[allow(clippy::too_many_lines)]
fn resolve_impl(
    ctx: &dyn EvalCtx,
    src: CellId,
    left: CellId,
    top: CellId,
    depth: u32,
) -> Option<CellId> {
    if depth > 256 {
        return None; // 防御：产品无界递归，此处不可能到达（head 图无环）
    }
    let d = depth + 1;
    let root = ctx.root();
    let left = if left.is_some() { left } else { root };
    let top = if top.is_some() { top } else { root };
    let left_src = ctx.source_of(left);
    let top_src = ctx.source_of(top);
    if left_src == src {
        return Some(left);
    }
    if top_src == src {
        return Some(top);
    }
    if is_left_head_of(ctx, left_src, src) && left_src != ctx.left_head(src) {
        for sub in ctx.left_subs(left) {
            if !is_left_head_of(ctx, ctx.source_of(sub), src) {
                continue;
            }
            for cell in ctx.ext_cells(sub) {
                if let Some(c) = resolve_impl(ctx, src, cell, top, d) {
                    return Some(c);
                }
            }
        }
    } else if is_top_head_of(ctx, top_src, src) && top_src != ctx.top_head(src) {
        for sub in ctx.top_subs(top) {
            if !is_top_head_of(ctx, ctx.source_of(sub), src) {
                continue;
            }
            for cell in ctx.ext_cells(sub) {
                if let Some(c) = resolve_impl(ctx, src, left, cell, d) {
                    return Some(c);
                }
            }
        }
    } else {
        if is_direct_left_head(ctx, left_src, src) && left_src != root {
            return via_left_subs(ctx, src, left, top);
        }
        if is_direct_top_head(ctx, top_src, src) && top_src != root {
            return via_top_subs(ctx, src, left, top);
        }
        if left_src == root && top_src == root {
            // `_$9.getExtensible() == 18` 分流（`:225-231`）
            return if ctx.extensible(src) == 18 {
                via_left_subs(ctx, src, left, top)
            } else {
                via_top_subs(ctx, src, left, top)
            };
        }
    }
    None
}

/// `_$3(left, top)`（`:144-159`）：扫 **left 的左子格列表**里出处==src 的实例，
/// 命中 `top.isTopHeadOf(cell)` 者返回。
fn via_left_subs(ctx: &dyn EvalCtx, src: CellId, left: CellId, top: CellId) -> Option<CellId> {
    for sub in ctx.left_subs(left) {
        if ctx.source_of(sub) != src {
            continue;
        }
        for cell in ctx.ext_cells(sub) {
            if is_top_head_of(ctx, top, cell) {
                return Some(cell);
            }
        }
    }
    None
}

/// `_$2(left, top)`（`:161-166`）：对称，扫 **top 的上子格列表**，
/// 判 `left.isLeftHeadOf(cell)`。
fn via_top_subs(ctx: &dyn EvalCtx, src: CellId, left: CellId, top: CellId) -> Option<CellId> {
    for sub in ctx.top_subs(top) {
        if ctx.source_of(sub) != src {
            continue;
        }
        for cell in ctx.ext_cells(sub) {
            if is_left_head_of(ctx, left, cell) {
                return Some(cell);
            }
        }
    }
    None
}
