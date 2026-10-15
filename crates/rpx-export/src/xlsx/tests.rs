//! `to_xlsx` 的钉子：**读回自洽**（M6-02 验收，spec-m6 §2.4）。
//!
//! 判据 = 写 → `calamine` 读回 → 逐格与**独立手写的期望**（`Data` 形态 + 字面量）
//! 比对。不复用 `payload_of`/`value_text` 自证；合并区以 0 起矩形断言
//! （与写库坐标口径一致，直观对账）。

use calamine::{Data, Dimensions, Reader, Xlsx};
use rpx_engine::arena::{CalcCell, CalcGrid, MergedCell};
use rpx_engine::value::Value;

use super::to_xlsx;

/// 搭一个网格：`(row, col, value)` 三元组。
fn grid(rows: i32, cols: i32, cells: Vec<CalcCell>) -> CalcGrid {
    let mut g = CalcGrid::new(rows, cols);
    g.cells = cells;
    g
}

fn text_cell(r: i32, c: i32, s: &str) -> CalcCell {
    CalcCell::bare(r, c, Value::Text(s.to_string()))
}

/// 读回的一本「账」：首个 sheet 的取值 + 合并区列表。
struct Book {
    range: calamine::Range<Data>,
    merged: Vec<Dimensions>,
}

/// 写 → 读回（字节进、账出）。
fn read(bytes: &[u8]) -> Book {
    let mut wb: Xlsx<_> = calamine::open_workbook_from_rs(std::io::Cursor::new(bytes))
        .expect("to_xlsx 的产出必须是可读回的 xlsx");
    let name = wb.sheet_names().first().expect("至少一个 sheet").clone();
    let range = wb.worksheet_range(&name).expect("sheet 可读");
    let merged = wb.merge_cells_by_sheet_name(&name).expect("合并区可读");
    Book { range, merged }
}

impl Book {
    /// 1 起坐标 → 读回值（越界/空白 ⇒ `Empty`）。
    fn at(&self, r: u32, c: u32) -> Data {
        self.range
            .get_value((r - 1, c - 1))
            .cloned()
            .unwrap_or(Data::Empty)
    }

    /// 合并区矩形（0 起，排序后）。
    fn rects(&self) -> Vec<((u32, u32), (u32, u32))> {
        let mut v: Vec<_> = self.merged.iter().map(|d| (d.start, d.end)).collect();
        v.sort();
        v
    }
}

// ------------------------------------------------------------------
// 值口径表（模块文档逐条对应）
// ------------------------------------------------------------------

#[test]
fn typed_values_read_back() {
    let cells = vec![
        CalcCell::bare(1, 1, Value::Int(100)),
        CalcCell::bare(1, 2, Value::Long(9_000_000_000)),
        CalcCell::bare(1, 3, Value::Short(3)),
        CalcCell::bare(1, 4, Value::Byte(2)),
        CalcCell::bare(1, 5, Value::Double(1.5)),
        CalcCell::bare(1, 6, Value::Float(2.5)),
        text_cell(1, 7, "区域"),
        CalcCell::bare(1, 8, Value::Bool(true)),
        CalcCell::bare(1, 9, Value::Null),
    ];
    let book = read(&to_xlsx(&grid(1, 9, cells)));
    assert!(book.rects().is_empty());
    // 数值族一律读回 Float（Excel 数字的存储本体 = f64）。
    assert_eq!(book.at(1, 1), Data::Float(100.0));
    assert_eq!(book.at(1, 2), Data::Float(9_000_000_000.0));
    assert_eq!(book.at(1, 3), Data::Float(3.0));
    assert_eq!(book.at(1, 4), Data::Float(2.0));
    assert_eq!(book.at(1, 5), Data::Float(1.5));
    assert_eq!(book.at(1, 6), Data::Float(2.5));
    assert_eq!(book.at(1, 7), Data::String("区域".to_string()));
    // Bool 是原生布尔（不是 "true" 文本）。
    assert_eq!(book.at(1, 8), Data::Bool(true));
    assert_eq!(book.at(1, 9), Data::Empty);
}

#[test]
fn decimal_number_or_text_fallback() {
    let cells = vec![
        CalcCell::bare(1, 1, Value::Decimal("3.5".to_string())),
        // 溢出 f64（parse 成功但 inf）⇒ 文本原样，不当数字写。
        CalcCell::bare(1, 2, Value::Decimal("1e999".to_string())),
        // 解析失败 ⇒ 文本原样（防御分支）。
        CalcCell::bare(1, 3, Value::Decimal("abc".to_string())),
    ];
    let book = read(&to_xlsx(&grid(1, 3, cells)));
    assert_eq!(book.at(1, 1), Data::Float(3.5));
    assert_eq!(book.at(1, 2), Data::String("1e999".to_string()));
    assert_eq!(book.at(1, 3), Data::String("abc".to_string()));
}

#[test]
fn non_finite_doubles_become_lowercase_text() {
    // 拦截写库的默认行为（会落 NAN/INF/-INF 全大写）：统一为 Rust Display 口径。
    let cells = vec![
        CalcCell::bare(1, 1, Value::Double(f64::NAN)),
        CalcCell::bare(1, 2, Value::Double(f64::INFINITY)),
        CalcCell::bare(1, 3, Value::Double(f64::NEG_INFINITY)),
    ];
    let book = read(&to_xlsx(&grid(1, 3, cells)));
    assert_eq!(book.at(1, 1), Data::String("NaN".to_string()));
    assert_eq!(book.at(1, 2), Data::String("inf".to_string()));
    assert_eq!(book.at(1, 3), Data::String("-inf".to_string()));
}

#[test]
fn disp_value_wins_over_value() {
    let mut both = CalcCell::bare(1, 1, Value::Int(5));
    both.disp_value = Some("5.00".to_string());
    let only_disp = CalcCell::display_only(1, 2, "仅显示值".to_string());
    let book = read(&to_xlsx(&grid(1, 2, vec![both, only_disp])));
    assert_eq!(book.at(1, 1), Data::String("5.00".to_string()));
    assert_eq!(book.at(1, 2), Data::String("仅显示值".to_string()));
}

#[test]
fn list_renders_as_joined_text() {
    let list = Value::List(vec![
        Value::Int(1),
        Value::Text("b".to_string()),
        Value::Null,
    ]);
    let book = read(&to_xlsx(&grid(1, 1, vec![CalcCell::bare(1, 1, list)])));
    // 与 text.rs 的逗号连接口径一致（"1,b,"）。
    assert_eq!(book.at(1, 1), Data::String("1,b,".to_string()));
}

#[test]
fn empty_text_reads_back_empty() {
    // 空串写库忽略 ⇒ 读回空（与 Null 读回同一形态）。
    let book = read(&to_xlsx(&grid(1, 1, vec![text_cell(1, 1, "")])));
    assert_eq!(book.at(1, 1), Data::Empty);
}

// ------------------------------------------------------------------
// 合并区：锚点重建 / 值 / 防御
// ------------------------------------------------------------------

#[test]
fn merged_area_reads_back_with_anchor_value_and_empty_slots() {
    let mut g = grid(
        6,
        6,
        // 区 A 锚点在 cells 里；区 B 锚点**不**在 cells（走 merged 记录的锚点值）。
        vec![text_cell(3, 1, "跨列标题")],
    );
    g.merged = vec![
        // 区 A：第 3 行跨 2 列。
        MergedCell::new(1, 3, 1, Value::Text("跨列标题".to_string()), 1, 2),
        MergedCell::new(1, 3, 2, Value::Text("跨列标题".to_string()), 1, 2),
        // 区 B：第 5-6 行第 5 列。
        MergedCell::new(1, 5, 5, Value::Int(42), 2, 1),
        MergedCell::new(1, 6, 5, Value::Int(42), 2, 1),
    ];
    let book = read(&to_xlsx(&g));
    assert_eq!(book.rects(), vec![((2, 0), (2, 1)), ((4, 4), (5, 4))]);
    assert_eq!(book.at(3, 1), Data::String("跨列标题".to_string()));
    assert_eq!(book.at(3, 2), Data::Empty); // 延续槽
    assert_eq!(book.at(5, 5), Data::Float(42.0));
    assert_eq!(book.at(6, 5), Data::Empty); // 延续槽
}

#[test]
fn continuation_records_rebuild_to_one_area() {
    // 逐槽视图：1 个区 × 3 槽 ⇒ xlsx 里必须只有 1 个 merge。
    let mut g = grid(2, 4, vec![]);
    g.merged = vec![
        MergedCell::new(1, 2, 2, Value::Text("A".to_string()), 1, 3),
        MergedCell::new(1, 2, 3, Value::Text("A".to_string()), 1, 3),
        MergedCell::new(1, 2, 4, Value::Text("A".to_string()), 1, 3),
    ];
    let book = read(&to_xlsx(&g));
    assert_eq!(book.rects(), vec![((1, 1), (1, 3))]);
    assert_eq!(book.at(2, 2), Data::String("A".to_string()));
}

#[test]
fn merged_anchor_display_value_wins() {
    // 锚点在 cells 里且带 disp ⇒ 与普通格同口径（disp 优先）。
    let mut with_disp = CalcCell::bare(1, 1, Value::Int(7));
    with_disp.disp_value = Some("甲乙丙".to_string());
    let mut g = grid(1, 3, vec![with_disp]);
    g.merged = vec![
        MergedCell::new(1, 1, 1, Value::Int(7), 1, 3),
        MergedCell::new(1, 1, 2, Value::Int(7), 1, 3),
        MergedCell::new(1, 1, 3, Value::Int(7), 1, 3),
    ];
    let book = read(&to_xlsx(&g));
    assert_eq!(book.rects(), vec![((0, 0), (0, 2))]);
    assert_eq!(book.at(1, 1), Data::String("甲乙丙".to_string()));
}

#[test]
fn pathological_overlap_keeps_first_area_no_panic() {
    // 引擎不产出重叠合并区；本测试钉「外部构造的病态输入也不 panic、保首见」。
    // 区 A [1..2]×[3..4] 与区 B [2..3]×[1..3] 交于 (2,3)，但两锚点互不在对方矩形内
    // ⇒ 两条都是「锚点」，靠 taken 去重丢后者（否则写库报 MergeRangeOverlaps）。
    let mut g = grid(3, 4, vec![]);
    let a = Value::Text("A".to_string());
    let b = Value::Text("B".to_string());
    g.merged = vec![
        MergedCell::new(1, 1, 3, a.clone(), 2, 2),
        MergedCell::new(1, 1, 4, a.clone(), 2, 2),
        MergedCell::new(1, 2, 3, a.clone(), 2, 2),
        MergedCell::new(1, 2, 4, a.clone(), 2, 2),
        MergedCell::new(1, 2, 1, b.clone(), 2, 3),
        MergedCell::new(1, 2, 2, b.clone(), 2, 3),
        MergedCell::new(1, 2, 3, b.clone(), 2, 3),
        MergedCell::new(1, 3, 1, b.clone(), 2, 3),
        MergedCell::new(1, 3, 2, b.clone(), 2, 3),
        MergedCell::new(1, 3, 3, b, 2, 3),
    ];
    let book = read(&to_xlsx(&g));
    assert_eq!(book.rects(), vec![((0, 2), (1, 3))]); // 只保区 A
    assert_eq!(book.at(1, 3), Data::String("A".to_string()));
}

// ------------------------------------------------------------------
// 边界（模块文档 §边界 逐条）
// ------------------------------------------------------------------

#[test]
fn text_beyond_excel_limit_is_clipped() {
    // Excel 硬上限 32,767 字符（写库判据同粒度）；多字节字符整字截断。
    let long = "汉".repeat(32_768);
    let book = read(&to_xlsx(&grid(1, 1, vec![text_cell(1, 1, &long)])));
    assert_eq!(book.at(1, 1), Data::String("汉".repeat(32_767)));
}

#[test]
fn empty_grid_still_reads_back() {
    let book = read(&to_xlsx(&CalcGrid::new(0, 0)));
    assert!(book.rects().is_empty());
    assert_eq!(book.at(1, 1), Data::Empty);
}

#[test]
fn out_of_extent_cells_ignored() {
    let g = grid(1, 1, vec![text_cell(1, 1, "x"), text_cell(3, 1, "越界")]);
    let book = read(&to_xlsx(&g));
    assert_eq!(book.at(1, 1), Data::String("x".to_string()));
    assert_eq!(book.at(3, 1), Data::Empty);
}

#[test]
fn cells_beyond_excel_limits_are_clipped() {
    // Excel 列上限 16,384（1 起）：16,300 界内、16,500 超界被裁剪。
    let g = grid(
        1,
        20_000,
        vec![
            CalcCell::bare(1, 16_300, Value::Int(1)),
            CalcCell::bare(1, 16_500, Value::Int(2)),
        ],
    );
    let book = read(&to_xlsx(&g));
    assert_eq!(book.at(1, 16_300), Data::Float(1.0));
    assert_eq!(book.at(1, 16_500), Data::Empty);
}

#[test]
fn merged_area_clipped_to_excel_limits() {
    // 锚点在界内、矩形跨界 ⇒ 裁剪到上限（不是整区丢弃）。
    let mut g = grid(1, 20_000, vec![]);
    g.merged = vec![MergedCell::new(1, 1, 1, Value::Int(5), 1, 20_000)];
    let book = read(&to_xlsx(&g));
    assert_eq!(book.rects(), vec![((0, 0), (0, 16_383))]);
}

#[test]
fn same_input_same_bytes() {
    // M6-04 的「CLI 产出与库函数直调逐字节一致」依赖写库 zip 时间戳固定。
    let g = grid(
        2,
        2,
        vec![
            text_cell(1, 1, "同输入"),
            CalcCell::bare(2, 2, Value::Int(7)),
        ],
    );
    assert_eq!(to_xlsx(&g), to_xlsx(&g));
}
