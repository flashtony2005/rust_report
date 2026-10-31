//! head 链归一化：把每格的 head 收到**最近的 `ExtExtendCell`**（§12.1 第 6 步）。
//!
//! 算法正文见 `docs/m2-engine-design.md` §4.4；本文件是
//! `ExtCellSet.java:783-806` 的逐行移植。
//!
//! # 为什么要这一步
//!
//! 阶段 B 第 2 步（[`super::find`]）算出的 head 可能指向**任意**格；
//! 扩展算法只认 `ExtExtendCell`。归一化沿 head 链上溯，
//! 把 head **替换**成第一个 `ExtExtendCell`；若绕回自己，则置为 `cell00`。
//!
//! # ⛔ 归一化必须**写回引用字段**（`setCellLeftHead`/`setCellTopHead`），
//! 不能只算不存 —— 后续 `extendRow`/`extendCol` 读的就是这些引用。
//!
//! ⚠ 本移植**未实现** `:790` 那条链的自环保护：Java 用 `left != sub` 终止，
//! 而本实现依赖 [`super::find::find_left_head`] 已保证链可达 `cell00`
//! （兜底 `return _$10`，`:665`）。若将来允许人工构造环，必须补回该保护。

use super::find::HeadGrid;

/// 归一化扫描的**尺寸上限**（对应 Java 的 `irow`/`icol`，即计算矩阵尺寸）。
///
/// Java 遍历 `1..irow` × `1..icol` 并跳过 `sub == null || sub.getRow() != i || sub.getCol() != j`
/// —— 即**只处理每行的主格**，非主格槽（合并块的副槽）直接跳过。
pub fn normalize_heads<G: HeadGrid + ?Sized>(g: &mut G, rows: i32, cols: i32) {
    let cell00 = g.cell00();
    for i in 1..rows {
        for j in 1..cols {
            let sub = g.get_calc(i, j);
            if sub.is_none() {
                continue;
            }
            // :787 只处理主格：副槽的 (row,col) 不等于槽坐标
            if g.row(sub) != i || g.col(sub) != j {
                continue;
            }
            // :788-797 左 head
            let left = g.left_head(sub);
            if !g.is_extender(left) {
                let mut cursor = left;
                let mut guard = 0;
                while !g.is_extender(cursor) && cursor != sub && guard < CHAIN_LIMIT {
                    cursor = g.left_head(cursor);
                    guard += 1;
                }
                // :792-796 绕回自己 ⇒ 置 cell00；否则置找到的 head
                let value = if sub == cursor { cell00 } else { cursor };
                g.set_left_head(sub, value);
            }
            // :798-804 上 head
            let top = g.top_head(sub);
            if !g.is_extender(top) {
                let mut cursor = top;
                let mut guard = 0;
                while !g.is_extender(cursor) && cursor != sub && guard < CHAIN_LIMIT {
                    cursor = g.top_head(cursor);
                    guard += 1;
                }
                g.set_top_head(sub, if sub == cursor { cell00 } else { cursor });
            }
        }
    }
}

/// 链遍历防御上限；与 `find.rs` 同值，避免两处常量漂移。
const CHAIN_LIMIT: usize = 4096;
