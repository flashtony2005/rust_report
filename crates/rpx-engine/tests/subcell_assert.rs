//! `sub_values`（附属格）的逐格断言：M2-09 覆盖面上仅剩的空白。
//!
//! # 存在意义
//!
//! `docs/spec-m2.md` §7 的 M2-09 要求 `calculated` 段**逐格**断言，其中包含
//! `source` 的 6 个子字段。`value` / `type` / `disp_value` 都已有专测，
//! **只剩 `sub_values` 的坐标与值从未被任何测试断言过** —— 解析侧若把附属格读错、
//! 或把 `sub_cells` 数错，现有测试全绿。本文件补这个洞。
//!
//! # 规模（现读，非转录）
//!
//! 10 个夹具里 **8 个**有 `sub_values`（`engine_ext` / `engine_samemerge` 只是其中两个），
//! 共 **9 个锚点 / 79 个附属格** —— 全部由下面的测试遍历现读，断言里不写死这些数字。
//!
//! # ⛔ 纪律一：包络公式是**实测归纳**，不是已证规则
//!
//! 我核对了全部 9 个锚点。`extend == 18`（纵向）时附属格的**行**恰好是
//! `src_row + k * row_merge`（k = 0..sub_cells），**列**恒为 `src_col`。
//! 注意**步长是 `row_merge` 而不是 1**：`engine_merge` 的锚点 `row_merge = 2`，
//! 三个附属格落在第 **2 / 4 / 6** 行（不是 2/3/4）。
//!
//! `extend == 19`（横向）时**列**是 `src_col + k * col_merge`，但**行不等于 `src_row`**
//! （`engine_ext` 的横向锚点 `src_row = 4`，附属格却全在第 **8** 行 —— 那一行由版面
//! 布局决定，**机制未核实**）。所以本文件**只断言它的列走向与"同属一行"**，
//! **不**断言它落在哪一行。
//!
//! ### 给后人的线索（**单点拟合，未核实，不要当规则写进断言**）
//!
//! team-lead 提供的假设：19 锚点的行号 = **其上方纵向扩展块占满之后的下一行**。
//! 代入 `engine_ext`：模板第 3 行是 18 锚点、扩展出 5 个子格（占第 3..7 行），
//! 于是 19 锚点落到第 **3 + 5 = 8** 行 —— 与实测一致。
//! 但**只有一个数据点**，且 `extendCol` 为何要下移尚无源码级证据。
//! 后人若能验证（补一个"纵向+横向共存且版面更复杂"的夹具，或读 `extendCol` 实现），
//! 请把结论写回这里并同步升级断言；**在此之前断言层保持只钉"同属一行 + 列走向"**。
//!
//! # ⛔ 纪律二：变异测试里"没失败"有两种含义
//!
//! 改一个值之后某个测试仍然通过，可能是①没被覆盖，也可能是②**不在变异路径上**。
//! 两者结论完全不同：本文件所有断言只依赖 `source` / `sub_values` 里的坐标与值，
//! **不依赖 `pagination`**，所以只要变异落在 `sub_values` 上，本文件应当**整体**变红；
//! 若出现"部分红部分绿"，按②解释（精确对应），**不要**当成漏网。
//!
//! # ⛔ 纪律三：变异必须**单因、逐个跑**
//!
//! **批量变异得到的"哪几个测试红了"不可信。** 本文件一次遍历**全部 10 个夹具**，
//! 而同一测试里先失败的夹具会 `panic` 掉整个循环 —— 后续夹具**根本没跑到**，
//! 它们的变异也就不会报红。上一轮我把两处变异分置两个夹具同时跑，值变异就被
//! `engine_expr`（按 `all_names()` 顺序更靠前）的坐标 panic **掩盖**了，
//! 只好把 `engine_expr` 还原后单独重跑，才拿到"只红 1 条"的干净证据。
//!
//! ⇒ **纪律：每次只改一处、只跑一遍、确认失败点与预期一致，再改下一处。**

mod common;

use common::{all_names, load, model::Grid, Cell};

/// 收集一个夹具里所有"带附属格"的锚点（`sub_cells > 1` 的格）。
fn anchors(g: &Grid) -> Vec<&Cell> {
    g.cells
        .iter()
        .filter(|c| c.source.as_ref().is_some_and(|s| s.sub_cells > 1))
        .collect()
}

/// 遍历全部夹具里带附属格的锚点，回调 `(夹具名, 该夹具的计算态网格, 锚点格)`。
fn for_each_anchor(mut f: impl FnMut(&str, &Grid, &Cell)) {
    for name in all_names() {
        let fx = load(name);
        // engine_types / engine_cycle 没有计算态网格，直接跳过
        let Some(g) = fx.grid() else { continue };
        for cell in anchors(g) {
            f(name, g, cell);
        }
    }
}

/// ① `len(sub_values) == sub_cells`；且 `sub_cells > 1` 时 `sub_values` 必非空。
///
/// 后半句是**响亮失败**：`sub_cells > 1` 却写不出 `sub_values`，说明 writer 漏了
/// （与 `CalcJson.java:84` 的 `if (subs > 1)` 矛盾）。
#[test]
fn sub_values_length_always_equals_sub_cells() {
    let (mut anchors_seen, mut subs_seen) = (0usize, 0usize);
    for_each_anchor(|name, _g, cell| {
        let s = cell.source.as_ref().unwrap();
        anchors_seen += 1;
        let sv = s.sub_values.as_ref().unwrap_or_else(|| {
            panic!("{name}: ({},{}) sub_cells={} 却缺 sub_values", cell.r, cell.c, s.sub_cells)
        });
        assert!(
            !sv.is_empty(),
            "{name}: ({},{}) sub_cells={} 但 sub_values 为空",
            cell.r,
            cell.c,
            s.sub_cells
        );
        assert_eq!(
            sv.len() as i32,
            s.sub_cells,
            "{name}: ({},{}) sub_values 长度应等于 sub_cells",
            cell.r,
            cell.c
        );
        subs_seen += sv.len();
    });
    assert!(anchors_seen > 0, "没有任何锚点带附属格 ⇒ 本测试是永真式");
    assert!(subs_seen > 0, "没有读到任何附属格 ⇒ 本测试是永真式");
}

/// ② 每个附属格都**落在计算态网格内**、**能在顶层 `cells` 里找到同坐标的格**、
/// 且 **`value` 与 `type` 完全相同**、坐标不重复。
///
/// 「值相同」钉的是：附属格是**同一个值重复下发**，不是逐格重新求值 ——
/// 若实现对每个附属格单独求值，值一旦随行变化就会红。
#[test]
fn every_sub_value_matches_the_top_level_cell_at_the_same_coordinate() {
    let mut checked = 0usize;
    for_each_anchor(|name, g, cell| {
        let s = cell.source.as_ref().unwrap();
        let mut seen: Vec<(i32, i32)> = Vec::new();
        for v in s.sub_values.as_ref().unwrap() {
            assert!(
                v.r >= 1 && v.r <= g.rows && v.c >= 1 && v.c <= g.cols,
                "{name}: 附属格 ({},{}) 越出计算态网格 {}x{}",
                v.r,
                v.c,
                g.rows,
                g.cols
            );
            let top = g
                .cell(v.r, v.c)
                .unwrap_or_else(|| panic!("{name}: 附属格 ({},{}) 在顶层 cells 里不存在", v.r, v.c));
            assert_eq!(
                v.value_raw, top.value_raw,
                "{name}: 附属格 ({},{}) 的值与顶层格不一致（应同为下发值，非重新求值）",
                v.r, v.c
            );
            assert_eq!(
                v.declared_type, top.declared_type,
                "{name}: 附属格 ({},{}) 的类型与顶层格不一致",
                v.r,
                v.c
            );
            assert!(
                !seen.contains(&(v.r, v.c)),
                "{name}: 附属格 ({},{}) 重复出现",
                v.r,
                v.c
            );
            seen.push((v.r, v.c));
            checked += 1;
        }
    });
    assert!(checked > 0, "没有比对任何附属格 ⇒ 本测试是永真式");
}

/// ③ 纵向锚点（`extend == 18`）的附属格：**同列**、行**严格升序**、**步长 = `row_merge`**、
/// 首行 **`>= src_row`**；且**同列的多个锚点之间行区间互不重叠**。
///
/// ## ⚠️ 本条规则已被新夹具**推翻过一次**，修正过程留在下面
///
/// 最初我归纳的是「行 = `src_row + k * row_merge`」，在当时的 8 个纵向锚点上 100% 吻合
/// （`engine_merge` 的 `row_merge = 2` 落在第 2/4/6 行，正好证明**步长是 `row_merge` 而非 1**）。
/// 加入 head 图夹具后 **`engine_head_left` 把它证伪了**：该夹具在同一列有**三个**纵向锚点
/// （模板第 2/3/4 行），实测附属格行分别是 **2-4 / 5-7 / 8-10** ——
///
/// ```text
/// anchor(2,1) src_row=2 sub_rows=[2,3,4]   ← 首行 == src_row（巧合：它是本列第一个）
/// anchor(3,1) src_row=3 sub_rows=[5,6,7]   ← 首行 5 ≠ 3
/// anchor(4,1) src_row=4 sub_rows=[8,9,10]  ← 首行 8 ≠ 4
/// ```
///
/// ⇒ **`src_row` 是模板态坐标**，而扩展带是**同列共享**的：后面的锚点被前面锚点占掉的
/// 行数**依次下推**。所以 `src_row` 只有在"本列第一个锚点"上才等于首行。
/// 这与「不能用模板行号索引计算态网格」「19 锚点行号由版面决定」是**同一类**陷阱。
///
/// 修正后的断言只保留在**全部 11 个纵向锚点**上实测成立的性质（逐条验证、零违反）：
/// 同列 / 严格升序且不重复 / 步长 = `row_merge` / 首行 `>= src_row` / 同列锚点区间不重叠。
#[test]
fn vertical_anchor_sub_values_run_down_the_column_with_step_row_merge() {
    let mut checked = 0usize;
    for name in all_names() {
        let fx = load(name);
        let Some(g) = fx.grid() else { continue };
        // 同列的纵向锚点收集起来，最后统一校验"依次下移、互不重叠"
        let mut by_col: Vec<(i32, i32, Vec<i32>)> = Vec::new();
        for cell in anchors(g) {
            let s = cell.source.as_ref().unwrap();
            if s.extend != 18 {
                continue; // 横向的行列走向不同，见另一个测试
            }
            let sv = s.sub_values.as_ref().unwrap();
            let rows: Vec<i32> = sv.iter().map(|v| v.r).collect();
            for v in sv {
                assert_eq!(
                    v.c, s.src_col,
                    "{name}: 纵向锚点 ({},{}) 的附属格应与锚点同列（src_col={}）", cell.r, cell.c, s.src_col
                );
            }
            for w in rows.windows(2) {
                assert!(w[1] > w[0], "{name}: 纵向锚点 ({},{}) 附属格行号应严格升序", cell.r, cell.c);
                assert_eq!(
                    w[1] - w[0], s.row_merge,
                    "{name}: 纵向锚点 ({},{}) 附属格行步长应为 row_merge={}", cell.r, cell.c, s.row_merge
                );
            }
            assert!(
                rows[0] >= s.src_row,
                "{name}: 纵向锚点 ({},{}) 首行 {} 不应小于模板行 src_row={}", cell.r, cell.c, rows[0], s.src_row
            );
            by_col.push((s.src_col, cell.r, rows));
            checked += sv.len();
        }
        by_col.sort();
        for pair in by_col.windows(2) {
            let (c1, _, prev) = &pair[0];
            let (c2, _, next) = &pair[1];
            if c1 != c2 {
                continue; // 不同列之间无先后关系
            }
            let lo = prev.iter().max().unwrap();
            let hi = next.iter().min().unwrap();
            assert!(
                lo < hi,
                "{name}: 同列 ({c1}) 相邻纵向锚点的附属格行区间不应重叠（前一个到 {lo}，后一个从 {hi}）"
            );
        }
    }
    assert!(checked > 0, "没有纵向锚点可比对 ⇒ 本测试是永真式");
}

/// ④ 横向锚点（`extend == 19`）：附属格**同属一行**，列是 `src_col + k * col_merge`。
///
/// ⛔ **不断言它落在哪一行** —— 实测 `engine_ext` 的横向锚点 `src_row = 4`，
/// 附属格却在第 8 行，说明横向带的行号由版面布局决定，**机制未核实**（同纪律一）。
#[test]
fn horizontal_anchor_sub_values_run_along_one_row() {
    let mut checked = 0usize;
    for_each_anchor(|name, _g, cell| {
        let s = cell.source.as_ref().unwrap();
        if s.extend != 19 {
            return;
        }
        let sv = s.sub_values.as_ref().unwrap();
        let band_row = sv[0].r;
        for (k, v) in sv.iter().enumerate() {
            assert_eq!(
                v.r, band_row,
                "{name}: 横向锚点 ({},{}) 的附属格应同属一行（实测第 {band_row} 行）",
                cell.r, cell.c
            );
            assert_eq!(
                v.c,
                s.src_col + k as i32 * s.col_merge,
                "{name}: 横向锚点 ({},{}) 第 {k} 个附属格列号应为 src_col + {k}*{}",
                cell.r, cell.c, s.col_merge
            );
            checked += 1;
        }
    });
    assert!(checked > 0, "没有横向锚点可比对 ⇒ 本测试是永真式");
}
