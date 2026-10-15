//! 复杂中式报表实例：sqlite 数据源 + 代码构造的非线性模板 + 扩展表达式。
//!
//! 运行：`cargo run -p rpx-server --example sqlite_chinese_report`
//!
//! 链路与 `rpx-cli render` / server `POST /export` **同构**：
//! 建 sqlite → 代码构造模板 → `load_datasets_full`（注入 sqlite 取数器）
//! → `calculate_with` → `paginate_with_setup` → 文本预览 + 导出 xlsx/csv。
//!
//! 演示的能力面（"扩展的表达"）：
//! * SQL 数据集取数（`ds1.select(col)` 纵向扩展、`ds1.sum/avg/count(col)` 聚合）；
//! * 单元格四则运算与聚合混用（`D4-E4`、`F4/E4`、`sum(ds1.select(...))`）；
//! * 属性表达式（行高 `rowHeight`、列宽 `colWidth`、断页 `breakPage`）；
//! * **同值合并·纵向**（`merge_same = 98`）——相邻等值并成一格；
//! * 多级合并表头 + 跨列标题 + 合计/平均行 —— 非线性版面。
//!
//! ⚠ 只用了求值器**已实现**的函数面：`sum/avg/max/min`、`ds1.select/sum/avg/count/col`、
//! 二元 `* - / ==`。名字表里虽有 `round`/`if`/`rmb`/`chn`，但求值器**未分派**它们，
//! 写进模板会得到 `UnsupportedExpr` —— 故本实例刻意避开。

mod layout;
mod seed;
mod template;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rpx_core::rowset::RowSet;
use rpx_ds::SqlSource;
use rpx_engine::arena::CalcGrid;
use rpx_engine::params::ParamTable;
use rpx_engine::{calculate_with, dataset::load_datasets_full, EngineError, Value};

fn main() {
    let out = demo_dir();
    std::fs::create_dir_all(&out).expect("建演示目录");
    let db = out.join("sales.db");
    // sqlx 的 AnyPool 未开 `create_if_missing` ⇒ 先落一个空文件再连
    // （sqlite 允许 0 字节文件，随后由 seed 建表）。
    std::fs::File::create(&db).expect("预建 sqlite 文件");
    // sqlx 绝对路径 URL 形态 = `sqlite:///abs/path`（三斜杠）。
    let url = format!("sqlite://{}", db.display());

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let src = rt
        .block_on(SqlSource::connect(&url))
        .unwrap_or_else(|e| panic!("连接 sqlite 失败 {url}：{e}"));
    let rows = rt.block_on(seed::seed(&src)).expect("种子数据");
    println!("[sqlite] 已建库 {}，sales 共 {rows} 行\n", db.display());

    // 代码构造模板（脱开 Windows 设计器）。
    let rd = layout::build_report(seed::SQL);

    // SQL 数据集取数：把 async 的 SqlSource 桥进引擎的同步 SqlFetcher 契约。
    let fetcher = |dsn: &str, sql: &str| -> Result<RowSet, EngineError> {
        let _ = dsn;
        rt.block_on(src.fetch_sql(sql))
            .map_err(|e| EngineError::SqlFetch {
                detail: e.to_string(),
            })
    };
    let ds = load_datasets_full(&rd, &|_| None, Some(&fetcher)).expect("载入数据集");

    // 计算 + 分页（与 CLI/server 同一条链路）。
    let mut grid = calculate_with(&rd, &ds, &ParamTable::EMPTY).expect("计算失败");
    let pg = rpx_engine::pager::paginate_with_setup(&grid, &rd.print_setup).expect("分页失败");
    grid.page_count = pg.page_count();
    grid.pagination = Some(pg);

    println!(
        "================ 渲染结果（{} 行 × {} 列）================",
        grid.rows, grid.cols
    );
    print_matrix(&grid);
    println!("\n分页：{} 页", grid.page_count);

    // 导出（与 CLI `render --out`、server `POST /export` 同一份导出器）。
    let xlsx = rpx_export::to_xlsx(&grid);
    let csv = rpx_export::to_csv(&grid);
    std::fs::write(out.join("sales_report.xlsx"), &xlsx).expect("写 xlsx");
    std::fs::write(out.join("sales_report.csv"), &csv).expect("写 csv");
    println!(
        "\n导出：\n  {}\n  {}（xlsx {} 字节，csv {} 字节）",
        out.join("sales_report.xlsx").display(),
        out.join("sales_report.csv").display(),
        xlsx.len(),
        csv.len()
    );

    assert_expected(&grid);
    println!("\n✅ 断言通过：合计/平均聚合 · 单元格运算 · 同值合并 · 行高/列宽属性 均符合预期");
}

/// 演示产物目录（系统临时目录，不入库）。
fn demo_dir() -> PathBuf {
    std::env::temp_dir().join("rpx_sqlite_demo")
}

/// 文本预览：按 (行,列) 铺开非空格值；合并矩形只在**左上锚点**打印一次。
///
/// ⚠ `grid.merged` 的每一项是合并矩形内的**每个槽**（含延续槽），且每项都带该矩形的
/// **整段跨度**（`row_merge`/`col_merge`）与锚点值 —— 结构里**没有**「谁是锚点」的字段。
/// 因此这里按 `(r,c)` 升序**贪心消费**矩形：同一矩形内最先遇到（最上、再最左）的槽
/// 就是它的左上锚点。逐槽外扩 `m.r..m.r+row_merge`（或按 `m.c` 外扩列）都**不对** ——
/// 延续槽自带的整段跨度会把相邻的真实数据格一起吞掉（实测吞掉首个明细行的 A/B/C 列）。
fn print_matrix(grid: &CalcGrid) {
    use std::collections::HashSet;

    let mut rects: Vec<(i32, i32, i32, i32)> = grid
        .merged
        .iter()
        .map(|m| (m.r, m.c, m.row_merge, m.col_merge))
        .collect();
    rects.sort_unstable(); // 行主序 ⇒ 每个矩形的左上锚点必先于其延续槽出现

    let mut taken: HashSet<(i32, i32)> = HashSet::new();
    let mut anchors: HashSet<(i32, i32)> = HashSet::new();
    for (r, c, rm, cm) in rects {
        if taken.contains(&(r, c)) {
            continue; // 已被先前矩形覆盖 ⇒ 本槽是延续槽
        }
        anchors.insert((r, c));
        for rr in r..r + rm {
            for cc in c..c + cm {
                taken.insert((rr, cc));
            }
        }
    }
    // 落在合并矩形内、但非锚点的槽 —— 预览不打印（否则跨列标题会逐列重复）。
    let mut merged_slots: HashSet<(i32, i32)> = HashSet::new();
    for m in &grid.merged {
        merged_slots.insert((m.r, m.c));
    }
    let is_continuation =
        |r: i32, c: i32| merged_slots.contains(&(r, c)) && !anchors.contains(&(r, c));

    let mut map: BTreeMap<(i32, i32), String> = BTreeMap::new();
    for cell in &grid.cells {
        if cell.row >= 1
            && cell.col >= 1
            && cell.is_emitted()
            && !is_continuation(cell.row, cell.col)
        {
            map.insert((cell.row, cell.col), fmt_value(&cell.value));
        }
    }
    let mut width = vec![0usize; grid.cols as usize + 1];
    for (&(_, col), s) in &map {
        width[col as usize] = width[col as usize].max(display_width(s));
    }
    for r in 1..=grid.rows {
        let mut line = String::from(" ");
        for c in 1..=grid.cols {
            let s = map.get(&(r, c)).map(String::as_str).unwrap_or("");
            line.push_str(s);
            for _ in 0..width[c as usize].saturating_sub(display_width(s)) {
                line.push(' ');
            }
            line.push_str(" │ ");
        }
        println!("{}", line.trim_end());
    }
}

/// 值 → 展示文本（整数不带 `.0`）。
fn fmt_value(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Int(n) => n.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Double(f) => trim_f(*f),
        Value::Float(f) => trim_f(*f as f64),
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => s.clone(),
        other => format!("{other:?}"),
    }
}

fn trim_f(f: f64) -> String {
    // 显示层保留 4 位小数（底层值仍全精度，断言用原值）。
    let r = (f * 10000.0).round() / 10000.0;
    if (r - r.round()).abs() < 1e-9 {
        format!("{}", r.round() as i64)
    } else {
        format!("{r}")
    }
}

/// 显示宽度：CJK 按 2 列计（终端对齐用）。
fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| if (c as u32) >= 0x1100 { 2 } else { 1 })
        .sum()
}

/// 断言关键计算值（把"能跑"升级成"跑对"）。
///
/// 期望值来自 `seed.rs` 的 15 行确定性数据，不是"只要是个数就算过"：
/// 合计本期 13610.5 / 上期 12720 / 增减额 890.5 / 平均本期 13610.5÷15。
fn assert_expected(grid: &CalcGrid) {
    let at = |r: i32, c: i32| -> Value {
        grid.cells
            .iter()
            .find(|x| x.row == r && x.col == c)
            .map(|x| x.value.clone())
            .unwrap_or(Value::Null)
    };
    let num = |v: &Value| -> Option<f64> {
        match v {
            Value::Int(n) => Some(*n as f64),
            Value::Long(n) => Some(*n as f64),
            Value::Double(f) => Some(*f),
            Value::Float(f) => Some(*f as f64),
            _ => None,
        }
    };
    let find_row = |text: &str| -> i32 {
        grid.cells
            .iter()
            .find(|c| matches!(&c.value, Value::Text(t) if t == text))
            .map(|c| c.row)
            .unwrap_or_else(|| panic!("应存在「{text}」行"))
    };

    // 明细扩展：15 行数据 ⇒ 明细区（模板第 4 行起）格子数应 > 15。
    let detail_cells = grid.cells.iter().filter(|c| c.row >= 4).count();
    assert!(
        detail_cells > 15,
        "明细应扩展成多行，实得 {detail_cells} 个格子"
    );

    // 合计行（模板第 5 行，扩展后落在明细之后）。
    let total_row = find_row("合计");
    let q3_total = num(&at(total_row, 4)).expect("合计·本期应为数值");
    let q2_total = num(&at(total_row, 5)).expect("合计·上期应为数值");
    let diff = num(&at(total_row, 6)).expect("合计·增减额应为数值");
    assert!(
        (q3_total - 13610.5).abs() < 1e-6,
        "合计本期应为 13610.5，实得 {q3_total}"
    );
    assert!(
        (q2_total - 12720.0).abs() < 1e-6,
        "合计上期应为 12720.0，实得 {q2_total}"
    );
    assert!(
        (diff - 890.5).abs() < 1e-6,
        "合计增减额应为 890.5，实得 {diff}"
    );
    println!("  合计行(row={total_row})：本期={q3_total} 上期={q2_total} 增减额={diff}");

    // 平均行（模板第 6 行）—— 再验一次聚合 + 单元格运算的组合。
    let avg_row = find_row("平均");
    let q3_avg = num(&at(avg_row, 4)).expect("平均·本期应为数值");
    assert!(
        (q3_avg - 13610.5 / 15.0).abs() < 1e-6,
        "平均本期应为 907.366…，实得 {q3_avg}"
    );
    println!("  平均行(row={avg_row})：本期={q3_avg}");

    // 行高属性表达式生效：`row_heights` 0 基（下标 i ↔ 逻辑第 i+1 行）。
    assert_eq!(
        grid.row_heights.first().copied(),
        Some(36.0),
        "标题行高应为 36"
    );
    assert_eq!(
        grid.row_heights.get(1).copied(),
        Some(22.0),
        "表头行高应为 22"
    );
    // 列宽属性表达式生效：第 1 列宽 = 14（`col_widths` 同样 0 基）。
    assert_eq!(
        grid.col_widths.first().copied(),
        Some(14.0),
        "第 1 列宽应为 14"
    );

    // 「同值合并·纵向」生效（`merge_same = 98`）——相邻等值并成一格。
    // 期望跨度：区域 华东/华南 各 6 行、华北 3 行；城市 上海/杭州/北京/广州/深圳 各 3 行。
    // 明细自第 4 行起：华东 4..=9、华北 10..=12、华南 13..=18；城市按 3 行一段。
    let span = |r: i32, c: i32| -> i32 {
        grid.merged
            .iter()
            .find(|m| m.r == r && m.c == c)
            .map(|m| m.row_merge)
            .unwrap_or(0)
    };
    for (r, want, what) in [(4, 6, "华东"), (10, 3, "华北"), (13, 6, "华南")] {
        assert_eq!(span(r, 1), want, "区域列「{what}」应纵向并成 {want} 行");
    }
    for (r, what) in [
        (4, "上海"),
        (7, "杭州"),
        (10, "北京"),
        (13, "广州"),
        (16, "深圳"),
    ] {
        assert_eq!(span(r, 2), 3, "城市列「{what}」应纵向并成 3 行");
    }
    println!("  同值合并：区域 6/3/6 行、城市各 3 行（merge_same=98 生效）");
}
