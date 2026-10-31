//! `merged_view` 的单测：两个投影的钉子。
//!
//! 数据形态复刻 `cross_emp_bonus` 真值（标题 1×9 合并 + 2×1 纵向合并 × 2 列，
//! 横向断页成 2 页）—— 投影逻辑的每条判据都对着真值形状钉。

use super::{global_slots, page_slots, MergedArea};
use crate::arena::MergedCell;
use crate::pager::{PageInfo, Pagination};
use crate::value::Value;

/// `cross_emp_bonus` 的合并区（全局坐标，1 起）：
/// 标题 1×9 + 纵向 2×1 × 4 列（第 8~9 行，真值即 4 列）。
fn emp_bonus_areas() -> Vec<MergedArea> {
    vec![
        MergedArea {
            r: 1,
            c: 1,
            row_merge: 1,
            col_merge: 9,
            value: Value::Text("T".into()),
        },
        MergedArea {
            r: 8,
            c: 1,
            row_merge: 2,
            col_merge: 1,
            value: Value::Int(1),
        },
        MergedArea {
            r: 8,
            c: 2,
            row_merge: 2,
            col_merge: 1,
            value: Value::Text("N".into()),
        },
        MergedArea {
            r: 8,
            c: 3,
            row_merge: 2,
            col_merge: 1,
            value: Value::Text("D".into()),
        },
        MergedArea {
            r: 8,
            c: 4,
            row_merge: 2,
            col_merge: 1,
            value: Value::Int(8500),
        },
    ]
}

/// 横向断页 2 页（x_pages=2，y_pages=1）：第 1 页 8 列、第 2 页 1 列，行全覆盖。
fn two_col_pages() -> Pagination {
    Pagination {
        paper_w: 595.0,
        paper_h: 842.0,
        columns: 1,
        page_count: 2,
        x_pages: 2,
        y_pages: 1,
        row_num_per_page: 20,
        pages: vec![
            PageInfo {
                panels: Vec::new(),
                page: 1,
                rows: 9,
                col1_values: Vec::new(),
                row_indices: (1..=9).collect(),
                row_range: (1, 9),
                col_indices: (1..=8).collect(),
                col_range: (1, 8),
            },
            PageInfo {
                panels: Vec::new(),
                page: 2,
                rows: 9,
                col1_values: Vec::new(),
                row_indices: (1..=9).collect(),
                row_range: (1, 9),
                col_indices: (9..=9).collect(),
                col_range: (9, 9),
            },
        ],
    }
}

/// 全局逐槽视图：每个合并区的每个槽各一条、带**整段**跨度（服务面快照的输入）。
///
/// 判据：标题 9 槽全 `col_merge=9`（不是页内的 8）；纵向合并 8 槽全 `row_merge=2`；
/// 总数 = 9 + 8 = 17（`engine_merge` 的「12 槽 = 6 块 × 2 槽」同形态）。
#[test]
fn global_slots_emit_every_slot_with_full_spans() {
    let slots = global_slots(&emp_bonus_areas());
    assert_eq!(slots.len(), 17, "标题 9 槽 + 纵向 8 槽");
    for m in slots.iter().filter(|m| m.r == 1) {
        assert_eq!((m.row_merge, m.col_merge), (1, 9), "标题槽必须是整段跨度");
        assert_eq!(m.page, 1, "全局视图 page 是占位 1");
    }
    for m in slots.iter().filter(|m| m.r >= 8) {
        assert_eq!((m.row_merge, m.col_merge), (2, 1), "纵向槽必须是整段跨度");
    }
    // 延续槽报锚点同值（`fill_rect` 共享 CellId 的投影等价物）
    assert!(slots
        .iter()
        .filter(|m| m.r == 1)
        .all(|m| m.value == Value::Text("T".into())));
}

/// 页内逐槽视图：`cross_emp_bonus` 真值的 16 条逐条复刻。
///
/// 三条判据（各踩过一个错）：
/// 1. 第 1 页 8 个标题槽**全部**报 `col_merge=8` —— 交矩形维度，不是整段 9，
///    也不是「本槽到页末还剩多宽」（按剩余宽度算是 8,7,6,5,4,3,2,1）；
/// 2. 第 2 页标题槽交矩形 `1×1` ⇒ 整条跳过（16 条不是 17 条）；
/// 3. 纵向合并不在断页方向上 ⇒ 两行都报 `row_merge=2`（4 条）。
#[test]
fn page_slots_clip_title_merge_across_column_pages() {
    let slots = page_slots(&emp_bonus_areas(), &two_col_pages());
    assert_eq!(
        slots.len(),
        16,
        "真值 16 条：标题 8 + 纵向 8（2 列 × 2 行 × 2 块）"
    );
    let title: Vec<&MergedCell> = slots.iter().filter(|m| m.page == 1 && m.r == 1).collect();
    assert_eq!(title.len(), 8, "第 1 页标题 8 槽");
    for (i, m) in title.iter().enumerate() {
        assert_eq!(m.c, i as i32 + 1, "页内列号按序");
        assert_eq!(m.col_merge, 8, "全部报页内交矩形维度 8（判据 1）");
        assert_eq!(m.value, Value::Text("T".into()));
    }
    assert!(
        slots.iter().all(|m| !(m.page == 2)),
        "第 2 页标题槽 1×1 被跳过（判据 2）"
    );
    let vertical: Vec<&MergedCell> = slots.iter().filter(|m| m.r >= 8).collect();
    assert_eq!(vertical.len(), 8, "纵向 4 块 × 2 行 = 8 条（判据 3）");
    for m in vertical {
        assert_eq!((m.row_merge, m.col_merge), (2, 1));
        assert_eq!(m.page, 1);
    }
}

/// 纵向断页（y_pages=2）：合并区跨行断开时，上页报「到页末」、下页报「从断行起」。
///
/// 复刻 `SimplePager._$1:1624-1638` 的拆分口径：锚点区 `setEndRow` 收窄到页末 +
/// deepClone 延续页新区。判据：上页 3 槽 `row_merge=3`、下页 2 槽 `row_merge=2`，
/// 下页坐标从 1 起（页局部）。
#[test]
fn page_slots_split_vertical_merge_across_row_pages() {
    let areas = vec![MergedArea {
        r: 4,
        c: 1,
        row_merge: 5,
        col_merge: 1,
        value: Value::Int(7),
    }];
    let pg = Pagination {
        paper_w: 595.0,
        paper_h: 842.0,
        columns: 1,
        page_count: 2,
        x_pages: 1,
        y_pages: 2,
        row_num_per_page: 20,
        pages: vec![
            PageInfo {
                panels: Vec::new(),
                page: 1,
                rows: 6,
                col1_values: Vec::new(),
                row_indices: (1..=6).collect(),
                row_range: (1, 6),
                col_indices: (1..=3).collect(),
                col_range: (1, 3),
            },
            PageInfo {
                panels: Vec::new(),
                page: 2,
                rows: 2,
                col1_values: Vec::new(),
                row_indices: (7..=8).collect(),
                row_range: (7, 8),
                col_indices: (1..=3).collect(),
                col_range: (1, 3),
            },
        ],
    };
    let slots = page_slots(&areas, &pg);
    assert_eq!(slots.len(), 5, "上页 3 行 + 下页 2 行");
    let up: Vec<&MergedCell> = slots.iter().filter(|m| m.page == 1).collect();
    assert_eq!(up.len(), 3);
    for m in up {
        assert_eq!(m.row_merge, 3, "上页段 = 行 4~6");
        assert_eq!(m.value, Value::Int(7), "延续克隆锚点同值");
    }
    let down: Vec<&MergedCell> = slots.iter().filter(|m| m.page == 2).collect();
    assert_eq!(down.len(), 2);
    for m in down {
        assert_eq!(m.row_merge, 2, "下页段 = 行 7~8");
        assert_eq!(m.value, Value::Int(7));
    }
    // 页局部坐标：上页行 4~6 → 4,5,6；下页行 7~8 → 1,2
    assert_eq!(
        slots.iter().map(|m| (m.page, m.r)).collect::<Vec<_>>(),
        vec![(1, 4), (1, 5), (1, 6), (2, 1), (2, 2)]
    );
}

/// 空页（`row_range == (0, 0)`，无区带也无数据行）必须跳过，不得凭空产出条目。
#[test]
fn page_slots_skips_empty_pages() {
    let areas = vec![MergedArea {
        r: 1,
        c: 1,
        row_merge: 2,
        col_merge: 2,
        value: Value::Null,
    }];
    let pg = Pagination {
        paper_w: 595.0,
        paper_h: 842.0,
        columns: 1,
        page_count: 2,
        x_pages: 1,
        y_pages: 2,
        row_num_per_page: 20,
        pages: vec![
            PageInfo {
                panels: Vec::new(),
                page: 1,
                rows: 2,
                col1_values: Vec::new(),
                row_indices: (1..=2).collect(),
                row_range: (1, 2),
                col_indices: (1..=2).collect(),
                col_range: (1, 2),
            },
            PageInfo {
                panels: Vec::new(),
                page: 2,
                rows: 0,
                col1_values: Vec::new(),
                row_indices: (0..=0).collect(),
                row_range: (0, 0),
                col_indices: (0..=0).collect(),
                col_range: (0, 0),
            },
        ],
    };
    assert_eq!(page_slots(&areas, &pg).len(), 4, "2×2 区 4 槽，空页无产出");
}
