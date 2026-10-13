//! 对拍器的索引小工具。**只索引真实出现的格** —— 这是纪律 ② 的落点：
//! `cells` 是稀疏的（`CalcJson.java:51-53`：`value` 与 `disp_value` 双 null 就不落盘），
//! 所以按 `(r, c)` 建索引后取**并集**，绝不遍历 `rows × cols` 的全部位置。

use std::collections::BTreeMap;

use super::super::json::Json;

/// `cells[]` / `sub_values[]` / `merged.cells[]` 的坐标索引。
pub(super) type CellIdx<'a> = BTreeMap<(i32, i32), &'a Json>;
/// `merged.cells[]` 的坐标索引（多一个 page 维）。
pub(super) type MergedIdx<'a> = BTreeMap<(i32, i32, i32), &'a Json>;

/// 取节点的原文 token；缺键 ⇒ `"<无此键>"`。
pub(super) fn raw_of(v: Option<&Json>) -> String {
    v.map(|x| x.raw.clone()).unwrap_or_else(|| "<无此键>".to_string())
}

/// 整数 token → i32。**只解析整数形态**的 token（`1.0` 会 panic，符合"不做归一化"）。
pub(super) fn as_i32(v: &Json) -> i32 {
    v.raw
        .parse::<i32>()
        .unwrap_or_else(|_| panic!("`{}` 不是整数 token（对拍器不做数值归一化）", v.raw))
}

pub(super) fn rc_of(cell: &Json, what: &str) -> (i32, i32) {
    (
        as_i32(cell.get("r").unwrap_or_else(|| panic!("{what} 必有 r"))),
        as_i32(cell.get("c").unwrap_or_else(|| panic!("{what} 必有 c"))),
    )
}

/// `calculated.rows` / `cols`；不是网格形态 ⇒ `None`。
pub(super) fn grid_dims(calc: &Json) -> Option<(i32, i32)> {
    let r = calc.get("rows")?;
    let c = calc.get("cols")?;
    Some((as_i32(r), as_i32(c)))
}

/// `cells[]` → `[(r, c)]` 索引。
pub(super) fn cell_index(cells: Option<&Json>) -> CellIdx<'_> {
    let mut m = CellIdx::new();
    if let Some(arr) = cells {
        for cell in arr.arr() {
            m.insert(rc_of(cell, "cell"), cell);
        }
    }
    m
}

/// `source.sub_values[]` → `[(r, c)]` 索引（缺键 ⇒ 空索引）。
pub(super) fn sub_index(sv: Option<&Json>) -> CellIdx<'_> {
    let mut m = CellIdx::new();
    if let Some(arr) = sv {
        for v in arr.arr() {
            m.insert(rc_of(v, "sub_value"), v);
        }
    }
    m
}

/// `merged.cells[]` → `[(page, r, c)]` 索引。
pub(super) fn merged_index(cells: Option<&Json>) -> MergedIdx<'_> {
    let mut m = MergedIdx::new();
    if let Some(arr) = cells {
        for v in arr.arr() {
            m.insert(
                (
                    as_i32(v.get("page").unwrap_or_else(|| panic!("merged cell 必有 page"))),
                    as_i32(v.get("r").unwrap_or_else(|| panic!("merged cell 必有 r"))),
                    as_i32(v.get("c").unwrap_or_else(|| panic!("merged cell 必有 c"))),
                ),
                v,
            );
        }
    }
    m
}

/// 每页的 `rows` token 序列（按页序）。
pub(super) fn page_rows(p: &Json) -> Vec<String> {
    p.get("pages")
        .map(|pg| pg.arr().iter().map(|x| raw_of(x.get("rows"))).collect())
        .unwrap_or_default()
}

/// 两侧索引的**坐标并集**（升序）。纪律 ② 的核心：只比"至少一侧出现"的坐标。
pub(super) fn union_keys<K: Ord + Copy>(a: &BTreeMap<K, &Json>, b: &BTreeMap<K, &Json>) -> Vec<K> {
    let mut keys: Vec<K> = a.keys().copied().collect();
    for k in b.keys() {
        if !a.contains_key(k) {
            keys.push(*k);
        }
    }
    keys.sort();
    keys
}
