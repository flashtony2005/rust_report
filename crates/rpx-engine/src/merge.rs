//! 同值合并 `mergeSame` 与空格合并 `mergeNull` —— `ExtCellSet` 的两个后处理趟。
//!
//! # 它不在 `calculate` 里
//!
//! 产品把合并放在**分页器**里，不在 `Engine.calc()`：
//! `SimplePager.createPages()` 的单页分支 `:133-134`（`mergeSame(); mergeNull();`）
//! 与多页分支 `:179-180`，另有 `CalcSubRpt:138-139`。所以「合并」是**分页态**的产物，
//! 而 `merged` 夹具段也是从 `PageBuilder` 的页上读的（`CalcMergeJson.java:34-63`）。
//!
//! ⓘ **历史实现边界已于 2026-10-14 更新**：早期在分页器落地前，两趟先跑在**计算矩阵**上，
//! 前提是当时的合并夹具全为单页（`docs/m2-engine-design.md` §8.1）；M2-11 分页器现已接入，
//! 但 `merge_same` / `merge_null` 仍由 `calculate()` 在分页前调用，尚未迁入分页态的每页矩阵。
//! 现有 12 夹具中，合并格样本仍是单页；**跨页合并格拆分**也仍是分页器的未实现边界
//! （见 `docs/m2-engine-design.md` §15.5）。因此「计算矩阵 ≡ 页网格」目前仍只对单页合并夹具成立，
//! 不许外推到未来的跨页合并样本。
//!
//! # 两张矩阵里改哪一张
//!
//! 两趟都只动**计算矩阵** `_$26`（本 crate 的 [`HeadState::by_pos`]）：
//!
//! | Java | 本 crate |
//! |------|----------|
//! | `_$26.get(i, j)` | `by_pos.get(&(i, j))` |
//! | `_$26.set(i, j, cell)` | `by_pos.insert((i, j), id)` —— **只改槽位映射** |
//! | `cell.getRow()/getCol()` | `cells[id].row/col` —— **永远是锚点自己的坐标** |
//! | `cell.setRowMerge(n)` / `addRowMerge(n)` | `cells[id].row_merge = n` / `+= n` |
//! | `cell.addRow(-n)` / `addCol(-n)` | `cells[id].row -= n` / `col -= n` |
//!
//! ⛔ `set` 把锚点对象塞进延续槽，但**不改锚点自己的 `row`/`col`** —— 这正是
//! 「延续槽导出锚点的整段跨度」（`engine_samemerge` 的 4 行 = 2 锚点 × 2 槽）的成因。
//! 同理 `by_pos[(r,c)]` 的格 `row != r` 就是「该槽是别名」的判据，两趟的扫描
//! 守卫（`cell.getRow() != i3`）全靠它。
//!
//! # 方向码
//!
//! | 属性 | 关 | 取值 |
//! |------|----|------|
//! | `mergeSame` | 97 | **98 纵向 / 99 横向 / 100 双向** |
//! | `mergeSameMode` | 118 | 118 `getSource()` / 119 左·上邻 / 120 仅反向趟 |
//! | `mergeNull` | 113 | **114 UP / 115 LEFT / 116 DOWN / 117 RIGHT** |
//!
//! # `ExtCellSet.java:1968` 的残值 —— 逐字复刻，不是笔误
//!
//! `mergeSame` 的 P3（横向正向趟）在 `for` 之前有一句
//! `if (max2 >= colSize) continue block19;`，而 `max2` 是**方法级局部**
//! （`:1842` 声明），此时读的是 **P1 留下的残值**，不是本趟的初值。
//!
//! 两条独立证据说明它是**真实源码**而非反编译噪声：
//!
//! ① 两个不同 JAR 的 CFR 0.152 产物给出**同一句**（`reportlite` `:1968` /
//!    `report` `:2106`）；
//! ② CFR 行为探针（`D`/`E` 两变体，见 `docs/java-reference-map.md` §6.6）：
//!    **只有在嵌套循环里 `continue` 跳向「外层」循环**才会产出 `continue blockN;`
//!    （Java 里这**必须**写标签）；跳向本层循环一律产出 `continue;`。
//!
//! 后果（`docs/java-reference-map.md` §6.6 的 P3 控制流模拟实测）：P3 处理完第一个
//! 使跨度触到右边界（`max2 == colSize`）的行之后，**其后每一行的首个候选格都会命中
//! 该 `if` 并跳到下一行** ⇒ P3 从那一行起整体停摆。残值 ≥ `colSize` 时 P3 一行都不做。
//!
//! ⚠ 没有任何夹具能分辨这一点：**全部 12 个夹具都没设过 `mergeSame` 的 99/100**
//! （`EngineModelsP0.java` 只设了 `98`），P3/P4 与整个 `mergeNull` 都**无夹具覆盖**。
//! 这里逐字复刻，是因为将来若从产品补一个 `mergeSame=99` 的夹具，**产品会带着这个
//! 残值行为**；把它"修好"反而会与夹具对不上。
//!
//! # 算法正文
//!
//! `docs/format-contract.md` 附件 K.2（`mergeSame` 四趟表 + 守卫 + 模式语义）、
//! K.3（`mergeNull` 四趟表 + 方向码反直觉说明）。

#[cfg(test)]
mod matrix;
mod null;
mod same;
#[cfg(test)]
mod tripwire;

pub use null::merge_null;
pub use same::merge_same;

use crate::arena::CellId;
use crate::expr::num::val_eq;
use crate::heads::{HeadCell, HeadState};
use crate::value::Value;

/// `mergeSame` = 98：纵向。
pub(crate) const SAME_V: i8 = 98;
/// `mergeSame` = 99：横向。
pub(crate) const SAME_H: i8 = 99;
/// `mergeSame` = 100：双向。
pub(crate) const SAME_VH: i8 = 100;
/// `mergeSameMode` = 118：源格判据用 `getSource()`（默认值）。
pub(crate) const MODE_SOURCE: i8 = 118;
/// `mergeSameMode` = 119：源格判据用左/上邻格。
pub(crate) const MODE_NEIGHBOR: i8 = 119;
/// `mergeSameMode` = 120：**只走反向趟**（P2/P4）。
pub(crate) const MODE_REVERSE: i8 = 120;

/// `mergeNull` = 114：向上吸收。
pub(crate) const NULL_UP: i8 = 114;
/// `mergeNull` = 115：向左吸收。
pub(crate) const NULL_LEFT: i8 = 115;
/// `mergeNull` = 116：向下吸收。
pub(crate) const NULL_DOWN: i8 = 116;
/// `mergeNull` = 117：向右吸收。
pub(crate) const NULL_RIGHT: i8 = 117;

/// 一个格在扫描期的只读快照。
///
/// 存在的唯一理由：两趟都是「读一个格、判若干属性、然后写另一个格（甚至写回自己）」，
/// 直接持 `&HeadCell` 会与写冲突。快照把借用压在一个语句里。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Snap {
    pub row: i32,
    pub col: i32,
    pub row_merge: i32,
    pub col_merge: i32,
    pub merge_same: i8,
    pub merge_same_mode: i8,
    pub merge_null: i8,
    pub value_null: bool,
}

/// 取快照（越界 id 返回 `None`；正常路径不会发生）。
pub(crate) fn snap(g: &HeadState, id: CellId) -> Option<Snap> {
    let c: &HeadCell = g.get(id)?;
    Some(Snap {
        row: c.row,
        col: c.col,
        row_merge: c.row_merge,
        col_merge: c.col_merge,
        merge_same: c.merge_same,
        merge_same_mode: c.merge_same_mode,
        merge_null: c.merge_null,
        value_null: c.value.is_null(),
    })
}

/// `_$26.get(i, j)`。
pub(crate) fn get(g: &HeadState, r: i32, c: i32) -> Option<CellId> {
    g.by_pos.get(&(r, c)).copied()
}

/// `_$26.set(i, j, id)`：只改槽位映射，**不**动格的 `row`/`col`。
pub(crate) fn set(g: &mut HeadState, r: i32, c: i32, id: CellId) {
    g.by_pos.insert((r, c), id);
}

/// `getRowCell(r).getRowType()`（`:494-499` 读的是 `_$26.get(r, 0)`）。
///
/// `None` = 该行没有行头格。Java 会在这里 NPE（`:1860` 等无判空），
/// 而本 crate 的建格序保证 `1..row_size` 每行都有行头格 ⇒ **不可达**；
/// 真出现时按「行类型不同」处理（中止合并），并在下方注释里标明这是刻意的分歧。
pub(crate) fn row_type_of(g: &HeadState, r: i32) -> Option<i8> {
    get(g, r, 0).and_then(|id| g.get(id)).map(|c| c.row_type)
}

/// `getColCell(c).getColType()`（`:509-514` 读的是 `_$26.get(0, c)`）。
/// 列头格由 `stage_b::build_grid` 预铺（`col_type_at`），故这里只做一次取值。
pub(crate) fn col_type_of(g: &HeadState, c: i32) -> Option<i8> {
    get(g, 0, c).and_then(|id| g.get(id)).map(|c| c.col_type)
}

/// `Variant2.equals(o1, o2)`（`:460-473`）：先 `getSingleValue` 解包，再判等。
///
/// * 两侧都解成 `null` ⇒ 相等（**空 List 也解成 null**，`:88-90`）；
/// * 一侧 null ⇒ 不等；
/// * 否则走 [`val_eq`]（本 crate 既有的 `Variant2.compareObjects` 替身：
///   数值面比 `f64`、非数值面结构相等）。
///
/// ⚠ 与 Java 的一处**已声明近似**：`getSingleValue` 会先过 `Variant2.getValue`
/// 拆 `Variable`/`DispValue` 包装（`:86`），而计算矩阵里的值已是裸值，无需拆。
pub(crate) fn same_value(a: &Value, b: &Value) -> bool {
    match (single_value(a), single_value(b)) {
        (None, None) => true,
        (None, Some(_)) | (Some(_), None) => false,
        (Some(x), Some(y)) => val_eq(x, y),
    }
}

/// `Variant2.getSingleValue`（`:85-94`）：空 List → `null`；非空 List → 首元素；其余原样。
///
/// 实现已收敛到 [`crate::value::single_value`]（全仓唯一一份），这里只转发。
fn single_value(v: &Value) -> Option<&Value> {
    crate::value::single_value(v)
}

/// `ExtCellSet._$6(start, end, col):1790-1809` —— 纵向合并的**互斥守卫**。
///
/// 扫描 `1..col` 每一行的 `start` 列与 `end-1` 列：只要发现某个已存在的格
/// **横跨**了 `[start, end)`（左沿在 `start` 左边、右沿在 `end` 左边；或左沿不晚于
/// `start`、右沿晚于 `end`），就返回 `false` = 「本段不许缩短」。只有模式 118 会调它。
pub(crate) fn guard6(g: &HeadState, start: i32, end: i32, col: i32) -> bool {
    for j in 1..col {
        if let Some(c1) = get(g, start, j).and_then(|id| snap(g, id)) {
            let row2 = c1.row + c1.row_merge;
            if c1.row < start && row2 < end {
                return false;
            }
        }
        let Some(c1) = get(g, end - 1, j).and_then(|id| snap(g, id)) else {
            continue;
        };
        let row2 = c1.row + c1.row_merge;
        if c1.row <= start || row2 <= end {
            continue;
        }
        return false;
    }
    true
}

/// `ExtCellSet._$5(start, end, row):1811-1830` —— 横向合并的互斥守卫（[`guard6`] 的转置）。
pub(crate) fn guard5(g: &HeadState, start: i32, end: i32, row: i32) -> bool {
    for i in 1..row {
        if let Some(c1) = get(g, i, start).and_then(|id| snap(g, id)) {
            let col2 = c1.col + c1.col_merge;
            if c1.col < start && col2 < end {
                return false;
            }
        }
        let Some(c1) = get(g, i, end - 1).and_then(|id| snap(g, id)) else {
            continue;
        };
        let col2 = c1.col + c1.col_merge;
        if c1.col <= start || col2 <= end {
            continue;
        }
        return false;
    }
    true
}
