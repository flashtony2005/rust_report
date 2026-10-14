//! [`super`] 的算术与断页单测。
//!
//! # 这些测试钉的是什么
//!
//! 三条**算术**（纸张双重舍入 / 单位换算不取整 / 千分位容差）与**纵向切点**。
//! 纵向切点是分页器唯一"会算错页数"的地方，用**合成网格**驱动：
//! 形状取自 `engine_pager`（43 行 / 第 2 行是分组头 / 全 20mm）与 `engine_sum`
//! （8 行 / 全 8mm），期望值取自夹具 `pagination.pages[].rows` 的实测值。
//!
//! ⛔ 合成网格**只复刻几何**（行高/行类型/列宽），不复刻格内容 —— 切点只由几何决定。
//! 端到端的真夹具对拍在 `tests/engine_grid_compare.rs`。

use super::*;
use crate::value::Value;

/// 造一个只有几何、没有格的网格。
fn geom(rows: i32, cols: i32, h_mm: f32, w_mm: f32) -> CalcGrid {
    let mut g = CalcGrid::new(rows, cols);
    g.unit = 1;
    g.row_heights = vec![h_mm; rows as usize];
    g.row_types = vec![-91; rows as usize];
    g.row_visible = vec![true; rows as usize];
    g.row_break_page = vec![false; rows as usize];
    g.col_widths = vec![w_mm; cols as usize];
    g.col_visible = vec![true; cols as usize];
    g.col_break_page = vec![false; cols as usize];
    g
}

/// 每页的行数序列。
fn rows_per_page(ys: &YSplit) -> Vec<i32> {
    ys.pages.iter().map(|p| p.len() as i32).collect()
}

// =====================================================================
// 三条算术
// =====================================================================

/// 842pt / 595pt 的 pt→mm→pt 往返值必须**逐位**对上（float32 中转 + double 回程）。
///
/// 这是"拿 842.0 当页高会在 `engine_rowheight` 上多算一页"那条坑的定点：
/// 两个往返值都**小于**名义值，所以容差的方向是"实际可用高略小"。
#[test]
fn paper_size_round_trip_reproduces_the_float32_double_rounding() {
    let h = paper_size_pt(PAPER_H_PT);
    let w = paper_size_pt(PAPER_W_PT);
    assert_eq!(h, 841.9999730868603, "842pt 的往返值必须逐位对上");
    assert_eq!(w, 594.9999807763288, "595pt 的往返值必须逐位对上");
    assert!(h < 842.0 && w < 595.0, "两个往返值都必须**小于**名义值");
    // 反证不是"原样返回"：往返确实改了值
    assert_ne!(h, f64::from(PAPER_H_PT), "往返若恒等，本模块就没在复刻舍入");
}

/// `unitTransfer` 的三个分支：0 = 原值、1 = mm→pt、2 = 英寸→pt；且**都不取整**。
#[test]
fn unit_transfer_matches_java_for_every_unit_branch() {
    assert_eq!(unit_to_pt(25.0, 1), 70.86614173228347, "mm → pt");
    assert_eq!(unit_to_pt(1.0, 2), 72.0, "英寸 → pt");
    assert_eq!(unit_to_pt(25.0, 0), 25.0, "单位 0 原样");
    // `isHighPrecision == true` ⇒ 不 `Math.ceil`：0.3 若取整会变 1.0。
    assert_eq!(
        unit_to_pt(0.3, 0),
        f64::from(0.3_f32),
        "单位 0 原样（f32 升 f64）"
    );
    assert!(unit_to_pt(0.3, 0) < 1.0, "不得走 `Math.ceil` 分支");
}

/// 断页判据带千分位容差：差 0.0005pt 视为放得下，差 0.002pt 视为放不下。
#[test]
fn break_test_carries_the_thousandth_tolerance() {
    // 1000.0 - used 的余量恰好是 10.0005 ⇒ 容差内放得下
    assert!(fits(10.0004, 989.9996, 1000.0), "余量内应放得下");
    assert!(!fits(10.002, 989.9996, 1000.0), "超余量应断页");
    // 恰好相等也放得下（判据是严格 `>`）
    assert!(fits(10.0, 990.0, 1000.0), "恰好相等应放得下");
}

// =====================================================================
// 纵向切点（合成网格 = engine_pager / engine_sum 的几何）
// =====================================================================

/// `engine_pager` 的几何：43 行全 20mm、第 2 行是分组头（`-90`）⇒ 每页 `[14,14,14,4]`。
///
/// 第 2~4 页顶部各有一份**分组头副本**，它也吃页高、也计入该页行数。
#[test]
fn pager_shaped_grid_splits_into_14_14_14_4() {
    let mut g = geom(43, 3, 20.0, 25.0);
    g.row_types[1] = ROW_TYPE_GROUP_HEADER; // 第 2 行
    let ys = split_rows(&g, paper_size_pt(PAPER_H_PT));
    assert_eq!(rows_per_page(&ys), vec![14, 14, 14, 4]);

    // 第 1 页是源行 1..14，全是数据行
    let first: Vec<i32> = ys.pages[0].iter().map(|p| p.row).collect();
    assert_eq!(first, (1..=14).collect::<Vec<_>>());
    assert!(ys.pages[0].iter().all(|p| p.kind == PageRowKind::Data));

    // 第 2 页 = 1 份副本（源行 2）+ 源行 15..27
    let p2: Vec<(i32, PageRowKind)> = ys.pages[1].iter().map(|p| (p.row, p.kind)).collect();
    assert_eq!(p2[0], (2, PageRowKind::GroupCopy), "页顶必须是分组头副本");
    assert_eq!(p2.len(), 14);
    assert!(p2[1..].iter().all(|(_, k)| *k == PageRowKind::Data));

    // 第 4 页 = 1 份副本 + 源行 41..43
    let p4: Vec<i32> = ys.pages[3].iter().map(|p| p.row).collect();
    assert_eq!(p4, vec![2, 41, 42, 43]);
}

/// **副本也吃页高**：去掉分组头行会让第 2 页多塞一行 —— 反证副本不是白送的。
///
/// 注意两条序列的**末页**不同：有副本时末页 4 行（1 副本 + 3 源行），
/// 没有副本时末页只剩 1 行 —— 副本把源行往后挤了一格。
#[test]
fn invisible_rows_are_copied_but_do_not_consume_page_height() {
    let mut g = geom(3, 1, 150.0, 25.0);
    g.row_visible[1] = false;

    let pages = split_rows(&g, paper_size_pt(PAPER_H_PT));
    assert_eq!(rows_per_page(&pages), vec![2, 1]);
    assert_eq!(pages.pages[0][1].row, 2, "隐藏行仍在分页行序中");
}

#[test]
fn row_break_page_forces_a_cut_after_its_row() {
    let mut g = geom(6, 1, 20.0, 25.0);
    g.row_break_page[1] = true;

    let pages = split_rows(&g, paper_size_pt(PAPER_H_PT));
    assert_eq!(rows_per_page(&pages), vec![2, 4]);
}

#[test]
fn column_break_page_forces_a_horizontal_cut_after_its_column() {
    let mut g = geom(1, 3, 20.0, 25.0);
    g.col_break_page[1] = true;

    assert_eq!(
        split_cols(&g, paper_size_pt(PAPER_W_PT)),
        vec![(1, 2), (3, 3)]
    );
}

#[test]
fn the_group_header_copy_consumes_height() {
    let mut with = geom(43, 3, 20.0, 25.0);
    with.row_types[1] = ROW_TYPE_GROUP_HEADER;
    let mut without = geom(43, 3, 20.0, 25.0);
    without.row_types[1] = -91;

    let a = rows_per_page(&split_rows(&with, paper_size_pt(PAPER_H_PT)));
    let b = rows_per_page(&split_rows(&without, paper_size_pt(PAPER_H_PT)));
    assert_eq!(a, vec![14, 14, 14, 4], "有分组头：3 份副本共占 3 行");
    assert_eq!(b, vec![14, 14, 14, 1], "没有分组头：43 = 14 + 14 + 14 + 1");

    // 直接钉住"副本吃高"：两边的第 2 页都装 14 行，但数据行数差 1。
    let data_of = |ys: &YSplit| -> Vec<usize> {
        ys.pages
            .iter()
            .map(|p| p.iter().filter(|r| r.kind == PageRowKind::Data).count())
            .collect()
    };
    assert_eq!(
        data_of(&split_rows(&with, paper_size_pt(PAPER_H_PT)))[1],
        13
    );
    assert_eq!(
        data_of(&split_rows(&without, paper_size_pt(PAPER_H_PT)))[1],
        14
    );
    assert_ne!(
        a, b,
        "副本若不吃页高，两种形状的切点会相同 ⇒ 本测试是永真式"
    );
}

/// **页首区带**（标题 `-96` / 表头 `-95`）每页都出现，且**高度先于数据行**累加。
///
/// 形状取自 `engine_head_left`：11 行、行 1 是 `-95`（表头）、行 8~10 是 `-96`（标题），
/// 夹具的 `pagination.rows` 是 **14** = 3 标题 + 1 表头 + 10 数据行。
/// 页内顺序是 **标题在前、表头在后**（`_$5:332-349` 的调用序）。
#[test]
fn page_prefix_bands_are_copied_onto_every_page() {
    let mut g = geom(11, 6, 8.0, 25.0);
    g.row_types = vec![-95, -91, -91, -91, -91, -91, -91, -96, -96, -96, -91];
    let ys = split_rows(&g, paper_size_pt(PAPER_H_PT));
    assert_eq!(
        rows_per_page(&ys),
        vec![14],
        "14 = 3 标题 + 1 表头 + 10 数据"
    );

    let got: Vec<(i32, PageRowKind)> = ys.pages[0].iter().map(|p| (p.row, p.kind)).collect();
    let head: Vec<(i32, PageRowKind)> = got
        .iter()
        .copied()
        .take_while(|(_, k)| *k == PageRowKind::Header)
        .collect();
    assert_eq!(
        head,
        vec![
            (8, PageRowKind::Header),
            (9, PageRowKind::Header),
            (10, PageRowKind::Header),
            (1, PageRowKind::Header),
        ],
        "页首区带顺序：标题(-96) 在前、表头(-95) 在后"
    );
    // 数据区是行 2..11（行 8~10 既是标题、也在数据循环里出现一次）
    let data: Vec<i32> = got
        .iter()
        .filter(|(_, k)| *k == PageRowKind::Data)
        .map(|(r, _)| *r)
        .collect();
    assert_eq!(data, (2..=11).collect::<Vec<_>>());
}

/// `engine_head_top` 的形状：行 2 是唯一的 `-96` ⇒ 页首 1 行 + 数据 6 行 = 7。
#[test]
fn a_single_title_row_prefixes_the_page() {
    let mut g = geom(6, 6, 8.0, 25.0);
    g.row_types = vec![-91, -96, -91, -91, -91, -91];
    let ys = split_rows(&g, paper_size_pt(PAPER_H_PT));
    assert_eq!(rows_per_page(&ys), vec![7]);
    let got: Vec<(i32, PageRowKind)> = ys.pages[0].iter().map(|p| (p.row, p.kind)).collect();
    assert_eq!(got[0], (2, PageRowKind::Header), "标题行复制到页顶");
    assert_eq!(
        got[1..].iter().map(|(r, _)| *r).collect::<Vec<_>>(),
        (1..=6).collect::<Vec<_>>(),
        "数据区仍是 1..6（含被复制的那一行）"
    );
}

/// 区带查找是**首个/末个连续段**，不是"所有该类型的行"。
#[test]
fn band_lookup_takes_contiguous_runs_only() {
    // 首个 -96 段是行 2（行 4 的那一段不参与）
    assert_eq!(leading_run(&[-91, -96, -96, -91, -96], -96), Some((2, 3)));
    // 末个 -96 段是行 5
    assert_eq!(trailing_run(&[-96, -91, -96, -96, -91], -96), Some((3, 4)));
    // 不存在 ⇒ None
    assert_eq!(leading_run(&[-91, -91], -96), None);
    assert_eq!(trailing_run(&[-91, -91], -96), None);
    // 数据区取首/末 -91/-90，**中间夹着的** -96 也算在范围内
    assert_eq!(data_rows(&[-95, -91, -96, -91, -91]), Some((2, 5)));
    assert_eq!(data_rows(&[-95, -96]), None, "全是区带 ⇒ 没有数据区");
}

/// `engine_sum` 的纵向几何：8 行全 8mm ⇒ 单页（横向另切）。
#[test]
fn sum_shaped_grid_stays_one_vertical_page() {
    let g = geom(8, 9, 8.0, 25.0);
    let ys = split_rows(&g, paper_size_pt(PAPER_H_PT));
    assert_eq!(rows_per_page(&ys), vec![8]);
}

/// 防御：单行高于整页时不死循环（产品此时抛 `highOutofPage`）。
#[test]
fn a_row_taller_than_the_page_terminates_instead_of_looping() {
    let mut g = geom(3, 1, 20.0, 25.0);
    g.row_heights = vec![400.0, 400.0, 400.0]; // 每行 400mm ≫ 297mm
    let ys = split_rows(&g, paper_size_pt(PAPER_H_PT));
    assert_eq!(
        rows_per_page(&ys),
        vec![1, 1, 1],
        "每行独占一页，且必须终止"
    );
}

// =====================================================================
// 横向切点
// =====================================================================

/// `engine_sum` 的横向几何：9 列全 25mm、可用宽 595pt（= 210mm）⇒ 前 8 列一页、第 9 列一页。
#[test]
fn nine_25mm_columns_split_into_8_plus_1() {
    let g = geom(8, 9, 8.0, 25.0);
    let spans = split_cols(&g, paper_size_pt(PAPER_W_PT));
    assert_eq!(spans, vec![(1, 8), (9, 9)]);
}

/// 宽到放得下时不切（单列页短路，`cols = [-1]` 的等价形态）。
#[test]
fn a_narrow_grid_stays_a_single_column_page() {
    let g = geom(8, 3, 8.0, 25.0); // 3 × 25mm = 75mm ≪ 210mm
    assert_eq!(split_cols(&g, paper_size_pt(PAPER_W_PT)), vec![(1, 3)]);
}

/// 零列网格返回一个空跨度，而不是 panic。
#[test]
fn an_empty_grid_yields_a_single_empty_span() {
    let g = CalcGrid::new(0, 0);
    assert_eq!(split_cols(&g, paper_size_pt(PAPER_W_PT)), vec![(1, 0)]);
    assert_eq!(
        rows_per_page(&split_rows(&g, paper_size_pt(PAPER_H_PT))),
        vec![0]
    );
}

// =====================================================================
// 逐页观测值
// =====================================================================

/// `col1_values` 取的是**该页自己的第 1 列**（翻页后不是网格第 1 列），
/// 且**含分组头副本**的值、跳过空值。
#[test]
fn col1_values_follow_the_pages_own_first_column_including_copies() {
    let mut g = geom(43, 3, 20.0, 25.0);
    g.row_types[1] = ROW_TYPE_GROUP_HEADER;
    // 只填第 1 列：行 1 = "region"、行 2 = "gh"、行 n = "Rn-2"；行 43 留空
    for r in 1..=42 {
        let v = match r {
            1 => "region".to_string(),
            2 => "gh".to_string(),
            n => format!("R{}", n - 2),
        };
        g.cells
            .push(crate::arena::CalcCell::bare(r, 1, Value::Text(v)));
    }
    let p = paginate(&g, PAPER_W_PT, PAPER_H_PT, COLUMNS);
    assert_eq!(p.page_count, 4);
    assert_eq!(p.x_pages, 1);
    assert_eq!(p.y_pages, 4);
    assert_eq!(p.row_num_per_page, 20);

    let rows: Vec<i32> = p.pages.iter().map(|q| q.rows).collect();
    assert_eq!(rows, vec![14, 14, 14, 4]);
    // 第 1 页首值 = 表头；第 2~4 页首值 = 副本带过来的分组头值
    let firsts: Vec<&Value> = p
        .pages
        .iter()
        .map(|q| q.col1_values.first().unwrap())
        .collect();
    assert_eq!(
        firsts,
        vec![
            &Value::Text("region".into()),
            &Value::Text("gh".into()),
            &Value::Text("gh".into()),
            &Value::Text("gh".into()),
        ]
    );
    // 第 4 页 4 行但只有 3 个非空首列值（第 43 行是空的）
    assert_eq!(p.pages[3].rows, 4);
    assert_eq!(p.pages[3].col1_values.len(), 3);
}

/// ⛔ 横向翻页后，「首列」是该页**自己的**第 1 列（`pg.getCell(r, 1)`），
/// 不是网格第 1 列 —— `engine_sum` 第 2 页取的是第 9 列的 `ds2Count` 即此。
///
/// **为什么必须单独有这条**：上面那条首列测试只造**单列页**（`base_col` 恒为 1），
/// 所以「把 `base_col` 写死成 1」这种错它**抓不到** —— 2026-10-13 的故障注入
/// 实测就是这样：注入在单列页上是**空操作**，两条门禁全绿。
/// 这条专造双列页，让该错在**单元层**就暴露。
#[test]
fn a_second_column_page_reads_its_own_first_column() {
    let mut g = geom(8, 9, 8.0, 25.0);
    for r in 1..=8 {
        g.cells.push(crate::arena::CalcCell::bare(
            r,
            1,
            Value::Text(format!("c1r{r}")),
        ));
        g.cells.push(crate::arena::CalcCell::bare(
            r,
            9,
            Value::Text(format!("c9r{r}")),
        ));
    }
    let spans = split_cols(&g, paper_size_pt(PAPER_W_PT));
    assert_eq!(
        spans,
        vec![(1, 8), (9, 9)],
        "列跨度必须以 1..=cols 的数据列坐标表示"
    );
    let p = paginate(&g, PAPER_W_PT, PAPER_H_PT, COLUMNS);
    assert_eq!(
        p.x_pages, 2,
        "9 列 × 25mm 放不下一页（8 列 = 566.9pt ≤ 595pt）"
    );
    assert_eq!(p.y_pages, 1, "8 行 × 8mm 只占一个纵向页");
    assert_eq!(p.page_count, 2);

    let cv: Vec<Vec<String>> = p
        .pages
        .iter()
        .map(|q| q.col1_values.iter().map(|v| format!("{v:?}")).collect())
        .collect();
    assert_eq!(cv[0].len(), 8, "第 1 页第 1 列 8 行全非空");
    assert_eq!(cv[1].len(), 8, "第 2 页第 9 列 8 行全非空");
    assert!(
        cv[0].iter().all(|s| s.contains("c1r")),
        "第 1 页应取**第 1 列**：{cv:?}"
    );
    assert!(
        cv[1].iter().all(|s| s.contains("c9r")),
        "第 2 页应取**第 9 列**（该页自己的第 1 列）：{cv:?}"
    );
    // 反证不是永真式：若两页取的是同一列，上面两条断言会同时成立。
    assert_ne!(cv[0], cv[1], "两页首列值相同 ⇒ 本测试是永真式");
}
