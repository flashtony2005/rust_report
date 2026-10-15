//! sqlite 建库 + 种子数据 + 模板 SQL。
//!
//! 一张明细表 `sales`：区域 / 城市 / 品类 × 本期(三季度) / 上期(二季度) / 成本
//! —— 覆盖 3 区域 × 5 城市 × 3 品类 = 15 行真实中文业务数据。

use rpx_ds::{DsError, SqlSource};

/// 模板里的数据集 SQL（`ORDER BY` 保证行序稳定，便于对拍）。
pub const SQL: &str = "SELECT region, city, category, q3_amount, q2_amount, cost \
                       FROM sales ORDER BY region, city, category";

const DROP: &str = "DROP TABLE IF EXISTS sales";

const SCHEMA: &str = "CREATE TABLE sales (\
    region    TEXT NOT NULL, \
    city      TEXT NOT NULL, \
    category  TEXT NOT NULL, \
    q3_amount REAL NOT NULL, \
    q2_amount REAL NOT NULL, \
    cost      REAL NOT NULL)";

const DATA: &str = "INSERT INTO sales (region, city, category, q3_amount, q2_amount, cost) VALUES \
    ('华东','上海','家电',1280.5,1150.0, 900.0), \
    ('华东','上海','食品', 640.0, 600.0, 480.0), \
    ('华东','上海','服饰', 520.0, 610.0, 380.0), \
    ('华东','杭州','家电', 980.0, 900.0, 700.0), \
    ('华东','杭州','食品', 410.0, 380.0, 300.0), \
    ('华东','杭州','服饰', 350.0, 300.0, 260.0), \
    ('华南','广州','家电',1450.0,1300.0,1020.0), \
    ('华南','广州','食品', 720.0, 690.0, 540.0), \
    ('华南','广州','服饰', 610.0, 560.0, 450.0), \
    ('华南','深圳','家电',1620.0,1500.0,1140.0), \
    ('华南','深圳','食品', 800.0, 760.0, 600.0), \
    ('华南','深圳','服饰', 690.0, 720.0, 510.0), \
    ('华北','北京','家电',1880.0,1700.0,1320.0), \
    ('华北','北京','食品', 900.0, 850.0, 670.0), \
    ('华北','北京','服饰', 760.0, 700.0, 560.0)";

/// 先删后建再插（幂等），并把行数读回来做断言。
pub async fn seed(src: &SqlSource) -> Result<usize, DsError> {
    src.fetch_sql(DROP).await?;
    src.fetch_sql(SCHEMA).await?;
    src.fetch_sql(DATA).await?;
    let rs = src.fetch_sql("SELECT COUNT(*) FROM sales").await?;
    Ok(rs.rows[0][0].to_string().parse().unwrap_or(0))
}
