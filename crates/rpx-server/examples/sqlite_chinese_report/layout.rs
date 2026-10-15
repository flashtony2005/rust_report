//! 报表版面：9 列 × 7 行的「区域销售业绩分析表」（非线性多级表头）。
//!
//! ```text
//! ┌───────────────────── 2026年三季度区域销售业绩分析表（合并 A1:I1，行高表达式=36）─────────────────────┐
//! ├──────┬──────┬──────┬──────────销售额(万元)──────────┬──────同比增减──────┬────────毛利(万元)────────┤
//! │ 区域 │ 城市 │ 品类 │      本期 │      上期 │ 增减额 │ 增减率 │ 本期毛利 │  毛利率  │
//! ├──────┼──────┼──────┼──────────┼──────────┼────────┼────────┼──────────┼──────────┤
//! │(扩展)│(扩展)│(扩展)│ ds.select│ ds.select│  D4-E4 │ F4/E4  │ q3-cost  │  H4/D4   │
//! ├──────┴──────┴──────┼──────────┼──────────┼────────┼────────┼──────────┼──────────┤
//! │        合计         │ ds.sum   │ ds.sum   │  D5-E5 │ F5/E5  │  sum(...)│  H5/D5   │  ← 断页属性表达式
//! ├─────────────────────┼──────────┼──────────┼────────┼────────┼──────────┼──────────┤
//! │        平均         │ ds.avg   │ ds.avg   │  D6-E6 │ F6/E6  │  avg(...)│  H6/D6   │
//! ├─────────────────────┴────────────────── 制表/审核/日期（合并 A7:I7，静态）──────────────────────────┤
//! ```
//!
//! 明细行的「区域」「城市」两列挂**同值合并·纵向**（`merge_same = 98`）——
//! 相邻等值的格并成一格，这是中式报表「合并相同项」的标准形态。

use rpx_core::ReportDefine;

use crate::template::{
    expr_cell, text_cell, Builder, KEY_COL_WIDTH, KEY_ROW_BREAK_PAGE, KEY_ROW_HEIGHT,
    MERGE_SAME_VERTICAL,
};

/// 数据源名（与 `seed.rs` 注入的 `dsn` 对齐）。
pub const DSN: &str = "sqlite";
/// 数据集名（模板声明；SQL 结果集不携带名字）。
pub const DATASET: &str = "ds1";

/// 构造报表模板。
pub fn build_report(sql: &str) -> ReportDefine {
    // rows/cols 含 0 号脚手架行/列 ⇒ 7 逻辑行 = 8 矩阵行，9 逻辑列 = 10 矩阵列。
    let mut b = Builder::new(8, 10);

    // ---- 第 1 行：标题（跨 9 列合并）+ 行高表达式 ----
    b.put(1, 1, text_cell("2026年三季度区域销售业绩分析表"));
    b.merge(1, 1, 1, 9);
    b.row_prop(1, KEY_ROW_HEIGHT, "36");

    // ---- 第 2 行：一级表头（多级合并：区域/城市/品类纵向，销售额/同比/毛利横向）----
    b.put(2, 1, text_cell("区域"));
    b.merge(2, 1, 3, 1);
    b.put(2, 2, text_cell("城市"));
    b.merge(2, 2, 3, 2);
    b.put(2, 3, text_cell("品类"));
    b.merge(2, 3, 3, 3);
    b.put(2, 4, text_cell("销售额（万元）"));
    b.merge(2, 4, 2, 5);
    b.put(2, 6, text_cell("同比增减"));
    b.merge(2, 6, 2, 7);
    b.put(2, 8, text_cell("毛利（万元）"));
    b.merge(2, 8, 2, 9);
    b.row_prop(2, KEY_ROW_HEIGHT, "22");

    // ---- 第 3 行：二级表头 ----
    b.put(3, 4, text_cell("本期"));
    b.put(3, 5, text_cell("上期"));
    b.put(3, 6, text_cell("增减额"));
    b.put(3, 7, text_cell("增减率"));
    b.put(3, 8, text_cell("本期毛利"));
    b.put(3, 9, text_cell("毛利率"));
    b.row_prop(3, KEY_ROW_HEIGHT, "22");

    // ---- 第 4 行：明细（ds1.select 驱动纵向扩展，一数据行一行）----
    // 区域/城市 挂「同值合并·纵向」⇒ 相邻等值并成一格（中式报表的"合并相同项"）。
    let region = b.put(4, 1, expr_cell("ds1.select(region)"));
    b.same_merge(region, MERGE_SAME_VERTICAL);
    let city = b.put(4, 2, expr_cell("ds1.select(city)"));
    b.same_merge(city, MERGE_SAME_VERTICAL);
    b.put(4, 3, expr_cell("ds1.select(category)"));
    b.put(4, 4, expr_cell("ds1.select(q3_amount)"));
    b.put(4, 5, expr_cell("ds1.select(q2_amount)"));
    b.put(4, 6, expr_cell("D4-E4")); // 增减额 = 本期 - 上期
    b.put(4, 7, expr_cell("F4/E4")); // 增减率 = 增减额 / 上期
    b.put(4, 8, expr_cell("ds1.select(q3_amount)-ds1.select(cost)")); // 本期毛利
    b.put(4, 9, expr_cell("H4/D4")); // 毛利率 = 毛利 / 销售额
    b.row_prop(4, KEY_ROW_HEIGHT, "18");

    // ---- 第 5 行：合计（并在其上强制分页，演示 breakPage 属性表达式）----
    b.put(5, 1, text_cell("合计"));
    b.merge(5, 1, 5, 3);
    b.put(5, 4, expr_cell("ds1.sum(q3_amount)"));
    b.put(5, 5, expr_cell("ds1.sum(q2_amount)"));
    b.put(5, 6, expr_cell("D5-E5"));
    b.put(5, 7, expr_cell("F5/E5"));
    b.put(
        5,
        8,
        expr_cell("sum(ds1.select(q3_amount))-sum(ds1.select(cost))"),
    );
    b.put(5, 9, expr_cell("H5/D5"));
    b.row_prop(5, KEY_ROW_HEIGHT, "20");
    b.row_prop(5, KEY_ROW_BREAK_PAGE, "1==1"); // 断页属性表达式（恒真 ⇒ 合计另起一页）

    // ---- 第 6 行：平均 ----
    b.put(6, 1, text_cell("平均"));
    b.merge(6, 1, 6, 3);
    b.put(6, 4, expr_cell("ds1.avg(q3_amount)"));
    b.put(6, 5, expr_cell("ds1.avg(q2_amount)"));
    b.put(6, 6, expr_cell("D6-E6"));
    b.put(6, 7, expr_cell("F6/E6"));
    b.put(6, 8, expr_cell("ds1.avg(q3_amount)-ds1.avg(cost)"));
    b.put(6, 9, expr_cell("H6/D6"));
    b.row_prop(6, KEY_ROW_HEIGHT, "20");

    // ---- 第 7 行：页脚（静态文本，跨 9 列合并）----
    b.put(
        7,
        1,
        text_cell("制表：财务部　　审核：__________　　日期：2026-10-16"),
    );
    b.merge(7, 1, 7, 9);

    // ---- 列宽属性表达式 ----
    b.col_prop(1, KEY_COL_WIDTH, "14");
    b.col_prop(2, KEY_COL_WIDTH, "14");
    b.col_prop(3, KEY_COL_WIDTH, "12");
    for c in 4..=9 {
        b.col_prop(c, KEY_COL_WIDTH, "16");
    }

    b.finish(DSN, DATASET, sql)
}
