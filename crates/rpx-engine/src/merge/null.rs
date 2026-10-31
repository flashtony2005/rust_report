//! `ExtCellSet.mergeNull():2051-2159` —— 四趟空格合并。
//!
//! 趟序与判据见 `docs/format-contract.md` 附件 K.3。与 `mergeSame` 的两处关键差别：
//!
//! 1. **判据在「被吸收方」身上**：`mergeSame` 要求两侧同值且都开了 `mergeSame`；
//!    `mergeNull` 只要求**被吸收的那一格**「值为空 + `mergeNull` 指向本趟方向」，
//!    锚点自己**不要求**开 `mergeNull`、也不要求值为空。
//! 2. **没有守卫**：`mergeNull` 不调 `_$5`/`_$6`，四趟都是直筒的 `for`。
//!
//! | 趟 | 行 | 扫描 | 吸收方向（判据码） | 锚点归属 |
//! |----|----|------|--------------------|----------|
//! | P1 | `:2069-2090` | `j3` 升、`i3` 升 | 向下吸 `114`(UP) | 上格 |
//! | P2 | `:2091-2113` | `j3` 升、`i3` 降 | 向上吸 `116`(DOWN) | **下格**（`addRow(-rm)` 上移） |
//! | P3 | `:2114-2135` | `i` 升、`j` 升 | 向右吸 `115`(LEFT) | 左格 |
//! | P4 | `:2136-2158` | `i` 升、`j` 降 | 向左吸 `117`(RIGHT) | **右格**（`addCol(-cm)` 左移） |
//!
//! ⚠ 方向码是**反直觉**的（附件 K.3）：判据码写的是**被吸收格的合并方向**，而吸收
//! 动作发生在**反方向**的那一格上 —— `114`(UP) 的格被**下方**的锚点吸走。
//!
//! ⛔ **全部 12 个夹具都没设过 `mergeNull`**（`EngineModelsP0.java` 未触碰），
//! 故本文件**无夹具级证据**，只有 `docs/format-contract.md` K.3 的源码级移植。
//! 补夹具前，这里的正确性由逐行对照 + 单测（合成矩阵）承担，不由对拍承担。

use super::*;

/// 四趟。调用方见 [`crate::merge`] 模块文档。
pub fn merge_null(g: &mut HeadState) {
    let row_size = g.row_size;
    let col_size = g.col_size;

    // ================= P1：向下吸 `114`(UP)（`:2069-2090`）=================
    for j3 in 1..col_size {
        let mut i3 = 1;
        while i3 < row_size {
            let Some(id) = get(g, i3, j3) else {
                i3 += 1;
                continue;
            };
            let s = snap(g, id).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
            if s.row != i3 || s.col != j3 {
                i3 += 1;
                continue;
            }
            let col_merge = s.col_merge;
            let rt = row_type_of(g, i3);
            let mut k = i3 + s.row_merge;
            while k < row_size {
                let Some(oid) = get(g, k, j3) else { break };
                let os = snap(g, oid).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
                if !os.value_null
                    || os.merge_null != NULL_UP
                    || os.col_merge != col_merge
                    || row_type_of(g, k) != rt
                {
                    break;
                }
                let rm = os.row_merge;
                for i2 in k..k + rm {
                    for j2 in j3..j3 + col_merge {
                        set(g, i2, j2, id);
                    }
                }
                g.cells[id.0 as usize].row_merge += rm;
                k += rm;
            }
            i3 += g.cells[id.0 as usize].row_merge;
        }
    }

    // ================= P2：向上吸 `116`(DOWN)（`:2091-2113`）=================
    for j3 in 1..col_size {
        let mut i3 = row_size - 1;
        while i3 >= 1 {
            let Some(id) = get(g, i3, j3) else {
                i3 -= 1;
                continue;
            };
            let s = snap(g, id).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
            if s.row != i3 || s.col != j3 {
                i3 -= 1;
                continue;
            }
            let col_merge = s.col_merge;
            let rt = row_type_of(g, i3);
            let mut k = i3 - 1;
            while k >= 1 {
                let Some(oid) = get(g, k, j3) else { break };
                let os = snap(g, oid).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
                if !os.value_null
                    || os.merge_null != NULL_DOWN
                    || os.col_merge != col_merge
                    || row_type_of(g, k) != rt
                {
                    break;
                }
                let rm = os.row_merge;
                let orow = os.row;
                // `cell.addRow(-rm)`：**锚点上移**到被吸收块的首行（`:2103`）。
                g.cells[id.0 as usize].row -= rm;
                for i2 in orow..i3 {
                    for j2 in j3..j3 + col_merge {
                        set(g, i2, j2, id);
                    }
                }
                g.cells[id.0 as usize].row_merge += rm;
                k -= rm;
            }
            i3 -= 1;
        }
    }

    // ================= P3：向右吸 `115`(LEFT)（`:2114-2135`）=================
    for i in 1..row_size {
        let mut j = 1;
        while j < col_size {
            let Some(id) = get(g, i, j) else {
                j += 1;
                continue;
            };
            let s = snap(g, id).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
            if s.row != i || s.col != j {
                j += 1;
                continue;
            }
            let row_merge = s.row_merge;
            let ct = col_type_of(g, j);
            let mut k = j + s.col_merge;
            while k < col_size {
                let Some(oid) = get(g, i, k) else { break };
                let os = snap(g, oid).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
                if !os.value_null
                    || os.merge_null != NULL_LEFT
                    || os.row_merge != row_merge
                    || col_type_of(g, k) != ct
                {
                    break;
                }
                let cm = os.col_merge;
                for i2 in i..i + row_merge {
                    for j2 in k..k + cm {
                        set(g, i2, j2, id);
                    }
                }
                g.cells[id.0 as usize].col_merge += cm;
                k += cm;
            }
            j += g.cells[id.0 as usize].col_merge;
        }
    }

    // ================= P4：向左吸 `117`(RIGHT)（`:2136-2158`）=================
    for i in 1..row_size {
        let mut j = col_size - 1;
        while j >= 1 {
            let Some(id) = get(g, i, j) else {
                j -= 1;
                continue;
            };
            let s = snap(g, id).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
            if s.row != i || s.col != j {
                j -= 1;
                continue;
            }
            let row_merge = s.row_merge;
            let ct = col_type_of(g, j);
            let mut k = j - 1;
            while k >= 1 {
                let Some(oid) = get(g, i, k) else { break };
                let os = snap(g, oid).unwrap_or_else(|| unreachable!("by_pos 必指向有效格"));
                if !os.value_null
                    || os.merge_null != NULL_RIGHT
                    || os.row_merge != row_merge
                    || col_type_of(g, k) != ct
                {
                    break;
                }
                let cm = os.col_merge;
                let ocol = os.col;
                // `cell.addCol(-cm)`：**锚点左移**到被吸收块的首列（`:2148`）。
                g.cells[id.0 as usize].col -= cm;
                for i2 in i..i + row_merge {
                    for j2 in ocol..j {
                        set(g, i2, j2, id);
                    }
                }
                g.cells[id.0 as usize].col_merge += cm;
                k -= cm;
            }
            j -= 1;
        }
    }
}
