//! `merged` 段的逐格断言：M2-09 覆盖面的最后一块。
//!
//! # 存在意义
//!
//! `calculated` 段的 `value` / `type` / `disp_value` / `source` / `sub_values` 都已有专测，
//! 只剩 `merged.cells[]`（`page` / `r` / `c` / `value` / `row_merge` / `col_merge`）
//! 的字段从未被任何测试断言过。
//!
//! # ⛔ 纪律一：`merged` 非空**不是**筛选条件，也**不是** mergeSame 的证据
//!
//! `merged` 在**全部**非错误型夹具里都**非 `null`**（`spec-m2.md` §7 第 ④ 条），
//! `cells` 为空是**常态**（10 个夹具里 6 个为空）。所以：
//! * 本文件**遍历全部夹具**，`merged` 为空只是"该夹具没有合并格"，**不是**跳过；
//! * **绝不能**把"某夹具 `merged` 非空"解释成"这里发生过 `mergeSame`" ——
//!   `engine_ext` 那 35 格全是 `row_merge=1 / col_merge=5`，是**模板列合并区**被
//!   19 横向扩展放大后的痕迹，与 `mergeSame` 无关。
//!
//! # ⛔ 纪律二：不要凭直觉给 `row_merge` 写"矩形锚点"规则
//!
//! 我实测过一个很自然的猜想并**证伪**了它：「同一页同列里，行区间重叠的两个合并格
//! 应当同值」。**反例 3 处**（`engine_merge` 列 1：`(3,1) rm=2 "North"` 与
//! `(4,1) rm=2 "East"` 行区间在第 4 行重叠但值不同；`engine_samemerge` 同样 1 处）。
//! ⇒ `merged` 里的 `row_merge` **不是**"以本格为左上角的矩形跨度"那么简单，
//! **机制未核实**。本文件因此**不对跨度几何写任何规则**，只钉可验证的值与拓扑。
//!
//! # ⛔ 纪律三：变异必须单因、逐个跑
//!
//! 批量变异得到的"哪几个测试红了"**不可信** —— 同一条测试里，先失败的夹具会
//! `panic` 掉循环，后续夹具的变异**根本没跑到**、也不会报红。上一轮我就因此
//! 让一次值变异的证据被更早的 panic 掩盖，只好单独重跑才拿到干净结果。
//!
//! # 多页下的坐标口径（已用真机探针核实，见 `tests/fixtures/ProbeMergePages.java`）
//!
//! 现有 3 个有内容的夹具都是**单页**，所以"页内坐标"与"全局坐标"恰好重合、从未被证伪。
//! 探针用两个变体（各 40 数据行、行高 20mm、595x842、unit=1）实测了多页情形：
//!
//! | 变体 | 页数 | 每页行数 | 每页合并格 | `r` 范围 | 判定 |
//! |---|---|---|---|---|---|
//! | A `mergeSame=98` 纵向 | 4 | 14/14/14/**1** | 12/12/12/**0** | [2,13] / [2,13] / [2,13] / — | **`r` 是页内坐标** |
//! | B 模板纵向合并 A2:A3 | 6 | 14×5 + 12 | 12×5 + 10 | [2,13]×5 / [2,11] | **`r` 是页内坐标** |
//!
//! ⇒ `merged.cells` 的 `r` **确实是页内坐标**（从 1 起、每页重新计数），
//! 与 `CalcMergeJson.java:37-59` 的读法一致，**契约无修正**。
//!
//! ⛔ 同时实测到两条**容易写错判据**的事实，先记在这里免得后人重犯：
//! * **`page` 不保证覆盖每一页** —— 变体 A 末页只有 1 行、无可合并 ⇒ 该页 **0** 个合并格。
//!   所以「每页都应出现在 `merged.cells` 里」是**错的**判据。
//! * **模板纵向合并会传播到每一页**，不只停在模板行所在的那一页（变体 B **6/6** 页都有合并格）。
//!   所以「合并只出现在模板行所在页」同样是**错的**判据。
//!
//! 多页夹具本身由后端登记（我不碰 `GenEngineFixtures.NAMES`）。上面两条已先行核实，
//! 夹具到位后可直接据此写断言，不必再猜。

mod common;

use common::{all_names, load};

/// ① `merged` 在非错误型夹具里**恒非 `null`**；错误型夹具里为 `None`；
/// 且 `merged.page_count` 恒等于 `pagination.page_count`。
///
/// ⚠️ 本测试**不**筛选"有合并格的夹具"—— 空 `cells` 的夹具同样要过这一关。
#[test]
fn merged_section_exists_for_every_non_error_fixture_and_agrees_on_page_count() {
    let mut checked = 0usize;
    for name in all_names() {
        let fx = load(name);
        match (&fx.merged, fx.is_error()) {
            // 错误型夹具：merged 必须是 null（spec §7 第 ④ 条）
            (None, true) => {
                assert!(fx.pagination.is_none(), "{name}: 错误型夹具 pagination 也应为 null");
            }
            (None, false) => panic!("{name}: 非错误型夹具的 merged 竟为 null"),
            (Some(m), false) => {
                let p = fx.pagination.as_ref().expect("非错误型夹具必有 pagination");
                assert_eq!(
                    m.page_count, p.page_count,
                    "{name}: merged.page_count 应与 pagination.page_count 一致"
                );
                assert_eq!(p.pages.len() as i32, p.page_count, "{name}: pages 长度应等于 page_count");
            }
            (Some(_), true) => panic!("{name}: 错误型夹具不该有 merged 段"),
        }
        checked += 1;
    }
    assert!(checked > 0, "没有遍历到任何夹具 ⇒ 本测试是永真式");
}

/// ② 每条被收录的合并格都**必须真的有跨度**，且坐标/页码合法、不重复。
///
/// `row_merge > 1 || col_merge > 1` 是 writer 过滤条件的**逆**：
/// `CalcMergeJson.java:48-50` 明确 `if (rm <= 1 && cm <= 1) continue;` ——
/// 两者都 ≤1 的格不落盘。所以落盘的格**必有**一个维度 >1。
#[test]
fn every_recorded_merged_cell_really_has_a_span() {
    let mut cells_seen = 0usize;
    for name in all_names() {
        let fx = load(name);
        let Some(m) = fx.merged.as_ref() else { continue };
        let mut seen: Vec<(i32, i32, i32)> = Vec::new();
        for c in &m.cells {
            assert!(
                c.row_merge > 1 || c.col_merge > 1,
                "{name}: ({},{}) 行({})列({})合并度都 ≤1，不该出现在 merged.cells 里",
                c.r,
                c.c,
                c.row_merge,
                c.col_merge
            );
            assert!(c.row_merge >= 1 && c.col_merge >= 1, "{name}: 合并度必须为正");
            assert!(
                c.page >= 1 && c.page <= m.page_count,
                "{name}: 页码 {} 越出 1..={}",
                c.page,
                m.page_count
            );
            assert!(c.r >= 1 && c.c >= 1, "{name}: ({},{}) 坐标必须为正", c.r, c.c);
            assert!(
                !seen.contains(&(c.page, c.r, c.c)),
                "{name}: ({},{}) 在第 {} 页重复出现",
                c.r,
                c.c,
                c.page
            );
            seen.push((c.page, c.r, c.c));
            cells_seen += 1;
        }
    }
    assert!(cells_seen > 0, "没有任何合并格被读到 ⇒ 本测试是永真式");
}

/// ③ 第 1 列的合并格，其值必须是**该页 `col1_values` 的子序列**（按行序）。
///
/// 这是 `merged` 与 `pagination` 之间唯一的**独立交叉校验**：`col1_values` 是分页器
/// 自己 dump 的第 1 列值序列（`CalcJson.java:121-131`），与 `merged` 的来源不同，
/// 两者对得上才说明合并格确实取自那一页的内容。
///
/// ⛔ 只校验**第 1 列** —— 其他列在 `pagination` 里没有独立 dump 可对照。
/// ⛔ 只断言**子序列**，**不断言**"第 r 行 ↔ 第 r-1 个元素"这种下标对应：
/// `engine_samemerge` 第 1 页 `rows = 7` 而 `col1_values` 只有 6 个元素
/// （空值被跳过），下标对应并不普遍成立。
#[test]
fn merged_column_one_values_are_a_subsequence_of_that_page_col1_values() {
    let mut checked = 0usize;
    for name in all_names() {
        let fx = load(name);
        let Some(m) = fx.merged.as_ref() else { continue };
        let p = fx.pagination.as_ref().expect("非错误型夹具必有 pagination");
        for page in &p.pages {
            let mut col1: Vec<&common::MergedCell> = m
                .cells
                .iter()
                .filter(|c| c.page == page.page && c.c == 1)
                .collect();
            col1.sort_by_key(|c| c.r);
            let mut it = page.col1_values.iter();
            for c in col1 {
                assert!(
                    it.any(|v| v == &c.value_raw),
                    "{name}: 第 {} 页 ({},{}) 的合并值 {:?} 不是该页 col1_values 的子序列",
                    page.page,
                    c.r,
                    c.c,
                    c.value_raw
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "没有比对任何第 1 列合并格 ⇒ 本测试是永真式");
}
