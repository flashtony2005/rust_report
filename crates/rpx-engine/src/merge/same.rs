//! `ExtCellSet.mergeSame():1832-2049` —— 四趟同值合并。
//!
//! 趟序与判据见 `docs/format-contract.md` 附件 K.2。四趟的骨架同构，只换轴与方向：
//!
//! | 趟 | 行 | 扫描 | 合并方向 | 进入模式 | 源格判据 |
//! |----|----|------|----------|----------|----------|
//! | P1 | `:1851-1902` | `j3` 升、`i3` 升 | 向下（`98`/`100`） | ≠ 120 | 118 `getSource()` / 119 左邻 |
//! | P2 | `:1903-1948` | `j3` 降、`i3` 降 | 向上（`98`/`100`） | = 120 | 右邻（`j3+1`） |
//! | P3 | `:1949-2002` | `i` 升、`j` 升 | 向右（`99`/`100`） | ≠ 120 | 118 `getSource()` / 119 上邻 |
//! | P4 | `:2003-2048` | `i` 降、`j` 降 | 向左（`99`/`100`） | = 120 | 下邻（`i+1`） |
//!
//! ⛔ P1 与 P3 共用一个方法级局部 `max2`（`:1842`），P3 的首个 `if` 读的是 **P1 的残值** ——
//! 见 [`super`] 模块文档「`:1968` 的残值」。这是**逐字复刻**，不许"顺手修好"。

use super::*;

/// 四趟。调用方见 [`crate::merge`] 模块文档（本 crate 暂在 `calc::run::calculate` 里调）。
pub fn merge_same(g: &mut HeadState) {
    let row_size = g.row_size;
    let col_size = g.col_size;
    // ⚠ 方法级共享局部（`ExtCellSet.java:1842`）：P1 写、P3 在赋值**之前**读。
    let mut max2: i32 = 0;

    // ================= P1：纵向 · 正向（`:1851-1902`）=================
    for j3 in 1..col_size {
        let mut i3 = 1;
        while i3 < row_size {
            let Some(id) = get(g, i3, j3) else {
                i3 += 1;
                continue;
            };
            let s = snap(g, id).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
            if s.value_null
                || s.row != i3
                || s.col != j3
                || (s.merge_same != SAME_V && s.merge_same != SAME_VH)
                || s.merge_same_mode == MODE_REVERSE
            {
                i3 += 1;
                continue;
            }
            let rt = row_type_of(g, i3);
            let col_merge = s.col_merge;
            let mode = s.merge_same_mode;
            let src = pick_src(g, mode, id, i3, j3 - 1, j3 > 1);

            max2 = i3 + s.row_merge;
            while max2 < row_size {
                let Some(oid) = get(g, max2, j3) else { break };
                let os = snap(g, oid).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
                if !same_value(
                    &g.cells[oid.0 as usize].value,
                    &g.cells[id.0 as usize].value,
                ) || os.col_merge != col_merge
                    || (os.merge_same != SAME_V && os.merge_same != SAME_VH)
                {
                    break;
                }
                if let Some(sv) = src {
                    let src2 = match mode {
                        MODE_SOURCE => as_opt(g.source_of(oid)),
                        MODE_NEIGHBOR => get(g, max2, j3 - 1),
                        // 模式 120 已被入口守卫排除（`:1856`），此处不可达。
                        _ => break,
                    };
                    if Some(sv) != src2 {
                        break;
                    }
                }
                if row_type_of(g, max2) != rt {
                    break;
                }
                max2 += os.row_merge;
            }
            // 守卫回退：`mode == 118` 且 `_$6` 判「可以缩」时才缩（`:1890-1892`）。
            while max2 >= i3 + s.row_merge
                && mode == MODE_SOURCE
                && !guard6(g, i3, max2, j3)
                && get(g, max2 - 1, j3) != Some(id)
            {
                let Some(oid) = get(g, max2 - 1, j3) else {
                    break;
                };
                let orm = snap(g, oid).map(|x| x.row_merge).unwrap_or(1);
                max2 -= orm;
            }
            let end = j3 + s.col_merge;
            for j2 in j3..end {
                for i22 in i3..max2 {
                    set(g, i22, j2, id);
                }
            }
            g.cells[id.0 as usize].row_merge = max2 - i3;
            i3 = max2;
        }
    }

    // ================= P2：纵向 · 反向，仅模式 120（`:1903-1948`）=================
    for j3 in (1..col_size).rev() {
        let mut i3 = row_size - 1;
        while i3 > 0 {
            let Some(id) = get(g, i3, j3) else {
                i3 -= 1;
                continue;
            };
            let s = snap(g, id).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
            if s.value_null
                || s.row != i3
                || s.col != j3
                || (s.merge_same != SAME_V && s.merge_same != SAME_VH)
                || s.merge_same_mode != MODE_REVERSE
            {
                i3 -= 1;
                continue;
            }
            let rt = row_type_of(g, i3);
            let col_merge = s.col_merge;
            let src = if j3 < col_size - 1 {
                get(g, i3, j3 + 1)
            } else {
                None
            };
            let max = i3 + s.row_merge;
            let mut k = i3 - 1;
            while k > 0 {
                let Some(oid) = get(g, k, j3) else { break };
                let os = snap(g, oid).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
                if !same_value(
                    &g.cells[oid.0 as usize].value,
                    &g.cells[id.0 as usize].value,
                ) || os.col_merge != col_merge
                    || (os.merge_same != SAME_V && os.merge_same != SAME_VH)
                {
                    break;
                }
                if let Some(sv) = src {
                    let src2 = if j3 < col_size - 1 {
                        get(g, k, j3 + 1)
                    } else {
                        None
                    };
                    if Some(sv) != src2 {
                        break;
                    }
                }
                if row_type_of(g, k) != rt {
                    break;
                }
                k -= os.row_merge;
            }
            // 锚点改为 `k + 1`（`:1938`）：循环没跑时 `k + 1 == i3`，仍是原格。
            let Some(aid) = get(g, k + 1, j3) else {
                i3 -= 1;
                continue;
            };
            let acm = snap(g, aid).map(|x| x.col_merge).unwrap_or(1);
            for j2 in j3..j3 + acm {
                for i23 in (k + 1)..max {
                    set(g, i23, j2, aid);
                }
            }
            g.cells[aid.0 as usize].row_merge = max - k - 1;
            i3 -= max - k - 1;
        }
    }

    // ================= P3：横向 · 正向（`:1949-2002`）=================
    // ⛔ `block19` 标签 + `continue block19`（`:1968`）：残值命中即**跳到下一行**。
    'block19: for i in 1..row_size {
        let mut j = 1;
        while j < col_size {
            let Some(id) = get(g, i, j) else {
                j += 1;
                continue;
            };
            let s = snap(g, id).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
            if s.value_null
                || s.row != i
                || s.col != j
                || (s.merge_same != SAME_H && s.merge_same != SAME_VH)
                || s.merge_same_mode == MODE_REVERSE
            {
                j += 1;
                continue;
            }
            let ct = col_type_of(g, j);
            let mode = s.merge_same_mode;
            let src = pick_src(g, mode, id, i - 1, j, i > 1);
            let row_merge = s.row_merge;
            // ⚠ 逐字复刻 `ExtCellSet.java:1968`：读的是 P1 留下的 `max2`。
            if max2 >= col_size {
                continue 'block19;
            }
            max2 = j + s.col_merge;
            while max2 < col_size {
                let Some(oid) = get(g, i, max2) else { break };
                let os = snap(g, oid).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
                if !same_value(
                    &g.cells[oid.0 as usize].value,
                    &g.cells[id.0 as usize].value,
                ) || os.row_merge != row_merge
                    || (os.merge_same != SAME_H && os.merge_same != SAME_VH)
                {
                    break;
                }
                if let Some(sv) = src {
                    let src2 = match mode {
                        MODE_SOURCE => as_opt(g.source_of(oid)),
                        MODE_NEIGHBOR => get(g, i - 1, max2),
                        // 模式 120 已被入口守卫排除（`:1955`），此处不可达。
                        _ => break,
                    };
                    if Some(sv) != src2 {
                        break;
                    }
                }
                if col_type_of(g, max2) != ct {
                    break;
                }
                max2 += os.col_merge;
            }
            while max2 > j + s.col_merge
                && mode == MODE_SOURCE
                && !guard5(g, j, max2, i)
                && get(g, i, max2 - 1) != Some(id)
            {
                let Some(oid) = get(g, i, max2 - 1) else {
                    break;
                };
                let ocm = snap(g, oid).map(|x| x.col_merge).unwrap_or(1);
                max2 -= ocm;
            }
            let end = i + s.row_merge;
            for i2 in i..end {
                for j22 in j..max2 {
                    set(g, i2, j22, id);
                }
            }
            g.cells[id.0 as usize].col_merge = max2 - j;
            j = max2;
        }
    }

    // ================= P4：横向 · 反向，仅模式 120（`:2003-2048`）=================
    for i in (1..row_size).rev() {
        let mut j = col_size - 1;
        while j > 0 {
            let Some(id) = get(g, i, j) else {
                j -= 1;
                continue;
            };
            let s = snap(g, id).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
            if s.value_null
                || s.row != i
                || s.col != j
                || (s.merge_same != SAME_H && s.merge_same != SAME_VH)
                || s.merge_same_mode != MODE_REVERSE
            {
                j -= 1;
                continue;
            }
            let ct = col_type_of(g, j);
            let src = if i < row_size - 1 {
                get(g, i + 1, j)
            } else {
                None
            };
            let row_merge = s.row_merge;
            let max = j + s.col_merge;
            let mut k = j - 1;
            while k > 0 {
                let Some(oid) = get(g, i, k) else { break };
                let os = snap(g, oid).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
                if !same_value(
                    &g.cells[oid.0 as usize].value,
                    &g.cells[id.0 as usize].value,
                ) || os.row_merge != row_merge
                    || (os.merge_same != SAME_H && os.merge_same != SAME_VH)
                {
                    break;
                }
                if let Some(sv) = src {
                    let src2 = if i < row_size - 1 {
                        get(g, i + 1, k)
                    } else {
                        None
                    };
                    if Some(sv) != src2 {
                        break;
                    }
                }
                if col_type_of(g, k) != ct {
                    break;
                }
                k -= os.col_merge;
            }
            let Some(aid) = get(g, i, k + 1) else {
                j -= 1;
                continue;
            };
            let arm = snap(g, aid).map(|x| x.row_merge).unwrap_or(1);
            for i2 in i..i + arm {
                for j23 in (k + 1)..max {
                    set(g, i2, j23, aid);
                }
            }
            g.cells[aid.0 as usize].col_merge = max - k - 1;
            j -= max - k - 1;
        }
    }
}

/// 源格判据（K.2 的 `src`）：模式 118 取 `getSource()`，模式 119 取邻格，120 不取。
///
/// `getSource()` 返回 `null` 时 Java 的 `src` 保持 `null`（`:1865` 是直接赋值），
/// 故这里把 [`CellId::NONE`] 折回 `None` —— 否则会与「邻格查不到」的 `None` 混为一谈。
fn pick_src(
    g: &HeadState,
    mode: i8,
    id: CellId,
    nr: i32,
    nc: i32,
    has_neighbor: bool,
) -> Option<CellId> {
    match mode {
        MODE_SOURCE => as_opt(g.source_of(id)),
        MODE_NEIGHBOR if has_neighbor => get(g, nr, nc),
        _ => None,
    }
}

/// [`CellId::NONE`] → `None`（Java 的 `null` 语义）。
fn as_opt(id: CellId) -> Option<CellId> {
    if id.is_none() {
        None
    } else {
        Some(id)
    }
}
