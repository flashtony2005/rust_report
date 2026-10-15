//! `to_csv` 的钉子（spec-m6 §2.3 逐条对应；判据是**字面量**不是自证）。

use rpx_engine::arena::{CalcCell, CalcGrid};
use rpx_engine::value::Value;

use super::to_csv;

/// 搭一个网格：`(row, col, value)` 三元组 + 可选 disp。
fn grid(rows: i32, cols: i32, cells: Vec<CalcCell>) -> CalcGrid {
    let mut g = CalcGrid::new(rows, cols);
    g.cells = cells;
    g
}

fn text_cell(r: i32, c: i32, s: &str) -> CalcCell {
    CalcCell::bare(r, c, Value::Text(s.to_string()))
}

// ------------------------------------------------------------------
// 规则 1：满矩阵展开（稀疏格落空、不留洞）
// ------------------------------------------------------------------

#[test]
fn full_matrix_expands_sparse_cells_with_empty_holes() {
    let g = grid(
        2,
        3,
        vec![text_cell(1, 1, "a"), CalcCell::bare(2, 2, Value::Int(5))],
    );
    // 字面量钉死：2 行 × 3 字段；CRLF 分隔；逗号补空；末尾无换行。
    assert_eq!(to_csv(&g), "a,,\r\n,5,");
}

#[test]
fn empty_grid_is_empty_string() {
    assert_eq!(to_csv(&CalcGrid::new(0, 0)), "");
    assert_eq!(to_csv(&CalcGrid::new(3, 0)), "");
    assert_eq!(to_csv(&CalcGrid::new(0, 3)), "");
}

#[test]
fn cells_outside_the_declared_extent_are_ignored() {
    // (3,1) 越界（rows=1）：不进输出也不 panic。
    let g = grid(1, 1, vec![text_cell(1, 1, "x"), text_cell(3, 1, "out")]);
    assert_eq!(to_csv(&g), "x");
}

// ------------------------------------------------------------------
// 规则 2：disp_value 优先；null ⇒ 空串
// ------------------------------------------------------------------

#[test]
fn disp_value_wins_over_value() {
    // engine_disp 的 (2,3) 形态：value=null、disp 非空。
    let only_disp = CalcCell::display_only(1, 1, "仅显示值".to_string());
    // 两者都有的形态：value=5、disp="5.00"。
    let mut both = CalcCell::bare(1, 2, Value::Int(5));
    both.disp_value = Some("5.00".to_string());
    let g = grid(1, 2, vec![only_disp, both]);
    assert_eq!(to_csv(&g), "仅显示值,5.00");
}

#[test]
fn null_value_renders_empty() {
    let g = grid(1, 2, vec![CalcCell::bare(1, 1, Value::Null)]);
    assert_eq!(to_csv(&g), ",");
}

// ------------------------------------------------------------------
// 规则 3：转义（RFC 4180 子集）
// ------------------------------------------------------------------

#[test]
fn fields_with_comma_quote_or_newline_are_quoted() {
    let cases: [(&str, &str); 5] = [
        ("plain", "plain"),
        ("a,b", "\"a,b\""),
        ("say \"hi\"", "\"say \"\"hi\"\"\""),
        ("l1\nl2", "\"l1\nl2\""),
        ("a\rb", "\"a\rb\""),
    ];
    for (raw, want) in cases {
        let g = grid(1, 1, vec![text_cell(1, 1, raw)]);
        assert_eq!(to_csv(&g), want, "raw={raw:?}");
    }
}

#[test]
fn list_joins_with_comma_then_gets_quoted() {
    let list = Value::List(vec![
        Value::Int(1),
        Value::Text("b".to_string()),
        Value::Null,
    ]);
    let g = grid(1, 1, vec![CalcCell::bare(1, 1, list)]);
    // 连接得 "1,b," ⇒ 含逗号 ⇒ 整段被引号包裹。
    assert_eq!(to_csv(&g), "\"1,b,\"");
}

// ------------------------------------------------------------------
// 文本口径：数值 shortest Display / Decimal 原样 / Bool 小写 / 非有限数保留文本
// ------------------------------------------------------------------

#[test]
fn numbers_render_shortest_and_decimal_is_verbatim() {
    let cells = vec![
        CalcCell::bare(1, 1, Value::Int(-7)),
        CalcCell::bare(1, 2, Value::Long(9_000_000_000)),
        CalcCell::bare(1, 3, Value::Double(8500.0)),
        CalcCell::bare(1, 4, Value::Double(1.5)),
        CalcCell::bare(1, 5, Value::Float(1.5)),
        CalcCell::bare(1, 6, Value::Short(3)),
        CalcCell::bare(1, 7, Value::Byte(2)),
        CalcCell::bare(1, 8, Value::Decimal("0.10".to_string())),
        CalcCell::bare(1, 9, Value::Bool(true)),
    ];
    assert_eq!(
        to_csv(&grid(1, 9, cells)),
        "-7,9000000000,8500,1.5,1.5,3,2,0.10,true"
    );
}

#[test]
fn non_finite_doubles_keep_text_instead_of_silent_empty() {
    let cells = vec![
        CalcCell::bare(1, 1, Value::Double(f64::NAN)),
        CalcCell::bare(1, 2, Value::Double(f64::INFINITY)),
        CalcCell::bare(1, 3, Value::Double(f64::NEG_INFINITY)),
    ];
    assert_eq!(to_csv(&grid(1, 3, cells)), "NaN,inf,-inf");
}

// ------------------------------------------------------------------
// 读回自洽对拍（M6-01 验收：导出 CSV 读回与 CalcGrid 逐格一致）
// ------------------------------------------------------------------

/// RFC 4180 子集的反解（引号包裹 + 内部 `""` 双写 + CRLF/LF 行尾）。
fn parse_csv(s: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        match ch {
            '"' => in_quotes = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(ch),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

#[test]
fn csv_reads_back_cell_by_cell() {
    // 混合形态：空位 / 转义 / 显示值 / 类型齐全。
    let mut with_disp = CalcCell::bare(3, 2, Value::Double(2.5));
    with_disp.disp_value = Some("2.50".to_string());
    let cells = vec![
        text_cell(1, 1, "区域"),
        CalcCell::bare(1, 2, Value::Int(100)),
        text_cell(2, 1, "含,逗号"),
        text_cell(2, 3, "带\"引号\"与\n换行"),
        with_disp,
        CalcCell::bare(4, 1, Value::Bool(false)),
    ];
    let g = grid(4, 3, cells);

    let csv = to_csv(&g);
    // 契约自查：CRLF 行尾、无末尾换行、无 BOM。
    assert!(csv.contains("\r\n"));
    assert!(!csv.ends_with('\n'));
    assert!(!csv.starts_with('\u{feff}'));

    let back = parse_csv(&csv);
    assert_eq!(back.len(), 4, "读回行数 = grid.rows");
    for r in 1..=g.rows as usize {
        assert_eq!(back[r - 1].len(), 3, "读回每行字段数 = grid.cols");
    }
    // 逐格与「期望文本」比对（期望值独立手写，不调 value_text 自证）。
    let want = [
        ["区域", "100", ""],
        ["含,逗号", "", "带\"引号\"与\n换行"],
        ["", "2.50", ""],
        ["false", "", ""],
    ];
    for (r, row) in want.iter().enumerate() {
        for (c, w) in row.iter().enumerate() {
            assert_eq!(&back[r][c], w, "({},{})", r + 1, c + 1);
        }
    }
}
