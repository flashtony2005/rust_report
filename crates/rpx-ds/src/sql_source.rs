//! SQL 数据源：`AnyPool` 取行 → [`rpx_core::rowset::RowSet`] 物化。
//!
//! # 链路（蓝图 §7.2 + `spec-m3.md` §3.4）
//!
//! ```text
//! url（postgres:// / mysql:// / sqlite://）
//!   → sqlx::any::install_default_drivers()（进程级，必须先调）
//!   → AnyPool::connect（按 URL 前缀选驱动，换库只换 URL）
//!   → sqlx::query(sql).fetch(&pool)   // 模板 SQL 原样透传
//!   → 逐行逐列 any_cell → Value
//!   → RowSet { name, cols = 结果集列名, rows }
//! ```
//!
//! # 三个必须认的坑（蓝图 §7.2 原文）
//!
//! ① `install_default_drivers()` 不调用，AnyPool 直接 panic；
//! ② AnyPool 没有 `query!` 宏级编译校验——报表 SQL 是用户模板带的，本来不受
//!    我们控制，正确性靠对拍与渲染期报错兜底；sqlx 0.9 起动态 SQL 会被
//!    `SqlSafeStr` lint 拦下，本 crate 按其官方口径用 `AssertSqlSafe(sql.to_owned())`
//!    显式豁免并在此留档（模板 SQL 是用户资产，注入面口径见 `spec-m3.md` §8）；
//! ③ SQLite 同一时刻只允许一个写者 ⇒ 池上限按库种配置（sqlite=1，网络库 10+）。

use rpx_core::rowset::RowSet;
use rpx_core::value::Value;
use sqlx::{Column, Executor, Row, SqlSafeStr, Statement, TypeInfo, ValueRef};

/// 取数错误。具体到可定位（渲染期报错兜底是蓝图口径，错误文本就是给用户的）。
#[derive(Debug)]
pub enum DsError {
    /// URL scheme 不在方言表（`dialect::for_url` 为 `None`）——显式报错不猜。
    UnsupportedScheme(String),
    /// 连接/查询失败（sqlx 原样透传）。
    Sql(sqlx::Error),
    /// 某列的值类型 Any 层解不出（骨架期未覆盖的列型，如实点名而非塞占位值）。
    UnsupportedColumnType {
        /// 列名。
        col: String,
        /// sqlx 报的类型名（如 `NUMERIC`）。
        type_name: String,
    },
}

impl std::fmt::Display for DsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DsError::UnsupportedScheme(url) => write!(f, "不支持的 URL scheme：{url}"),
            DsError::Sql(e) => write!(f, "SQL 取数失败：{e}"),
            DsError::UnsupportedColumnType { col, type_name } => {
                write!(f, "列 {col} 的类型 {type_name} 暂不支持（骨架期未覆盖）")
            }
        }
    }
}

impl std::error::Error for DsError {}

impl From<sqlx::Error> for DsError {
    fn from(e: sqlx::Error) -> Self {
        DsError::Sql(e)
    }
}

/// SQL 数据源。连接串即身份——同 URL 复用池（池注册表在 server 编排层做，
/// 本 crate 保持无状态：一个 `SqlSource` 对应一个池）。
pub struct SqlSource {
    pool: sqlx::pool::Pool<sqlx::any::Any>,
}

impl SqlSource {
    /// 建池并连接。sqlite 系 URL 强制 `max_connections = 1`（蓝图坑③）。
    pub async fn connect(url: &str) -> Result<Self, DsError> {
        crate::dialect::for_url(url).ok_or_else(|| DsError::UnsupportedScheme(url.to_string()))?;
        sqlx::any::install_default_drivers();
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(if url.starts_with("sqlite") { 1 } else { 10 })
            .connect(url)
            .await?;
        Ok(SqlSource { pool })
    }

    /// 执行非查询语句（DDL/DML），返回受影响行数。
    ///
    /// 测试基建用（`sqlite_fixture.rs` 内存库建表；SQLite 文件库的
    /// Windows URL 形态坑见下）——也是未来 CLI 初始化数据库的出口。（`sqlite_fixture.rs` 自建表，不依赖外部服务器）；也是
    /// 未来 CLI 初始化数据库的出口。SQL 安全性与 [`fetch_sql] 同规（模板侧
    /// 由服务端把关，此处不二次过滤）。
    pub async fn execute(&self, sql: &str) -> Result<u64, DsError> {
        let r = sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected())
    }

    /// 执行模板 SQL 并物化。列名取结果集表头（SQL 数据集的列 = 查询产物，
    /// 与产品 SQL 数据集语义一致），行值经 [`any_cell`] 收敛到 `Value`。
    pub async fn fetch_sql(&self, sql: &str) -> Result<RowSet, DsError> {
        // `fetch_all` 在没有行时拿不到列元数据；单独 prepare 一次取 statement.columns()。
        let cols = {
            let mut conn = self.pool.acquire().await?;
            let statement = conn
                .prepare(sqlx::AssertSqlSafe(sql.to_owned()).into_sql_str())
                .await?;
            statement
                .columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect::<Vec<_>>()
        };
        let rows: Vec<sqlx::any::AnyRow> = sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut vals = Vec::with_capacity(cols.len());
            for (i, col) in cols.iter().enumerate() {
                vals.push(any_cell(row, i).map_err(|e| match e {
                    DsError::UnsupportedColumnType { type_name, .. } => {
                        DsError::UnsupportedColumnType {
                            col: col.clone(),
                            type_name,
                        }
                    }
                    other => other,
                })?);
            }
            out.push(vals);
        }
        // name 由调用方（数据集配置）定；SQL 层只有行集。此处以 SQL 首词占位，
        // 由 server 编排层用配置名覆盖——见模块文档的职责边界。
        Ok(RowSet {
            name: String::new(),
            cols,
            rows: out,
        })
    }
}

/// 一格：Any 值 → `Value`（蓝图 §7.2 的类型映射收敛表）。
///
/// Any 层可解类型：bool/i16/i32/i64/f32/f64/String/Vec<u8>。级联 `try_get`
/// 逐型尝试——类型不匹配返回 Err 换下一型，全部落空时按**列类型名**报
/// [`DsError::UnsupportedColumnType`]（如实点名，不塞占位值）。
///
/// ⚠ 带账与真机实锤（金仓 KingbaseES 2026-10-15，`tests/smoke_pg.rs`）：
/// `NUMERIC`/日期等 Any 层**不认识**的类型在 `fetch_all` 解码期即 eager 报
/// `sqlx::Error::ColumnDecode("...Any driver does not support ...")`——
/// **到不了本函数的级联**（同 sqlite BOOLEAN，见测试注）；本函数尾部
/// `UnsupportedColumnType` 分支只对「Any 层认识、但九个 try_get 全落空」的
/// 类型可达。两条路都满足「显式点名、不塞占位值」契约。日期/时间列映射
/// 待 `SqlDataSetConfig` 反逆向一起定（`spec-m3.md` §8）。
fn any_cell(row: &sqlx::any::AnyRow, idx: usize) -> Result<Value, DsError> {
    // 整数族**统一 Long**：sqlx Any 层 `i64::compatible = ty.kind().is_integer()`
    // （sqlx-core-0.9 any/types/int.rs:15/43/71）——级联先试 i64，int2/int4/int8
    // 全兼容，恒落 `Long`（金仓 KingbaseES 真机 2026-10-15 实测同此，
    // `tests/smoke_pg.rs`）。下方 i32/i16 分支只是给非常规驱动的兜底，常规路走不到。
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return Ok(v.map(Value::Long).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<i32>, _>(idx) {
        return Ok(v.map(Value::Int).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<i16>, _>(idx) {
        return Ok(v.map(|x| Value::Int(i32::from(x))).unwrap_or(Value::Null));
    }
    // 浮点族：DOUBLE 优先，REAL → f32（Value 保留 Float 变体，不归一——M2-14 契约）。
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return Ok(v.map(Value::Double).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<f32>, _>(idx) {
        return Ok(v.map(Value::Float).unwrap_or(Value::Null));
    }
    // 布尔（PG bool；MySQL tinyint(1) 走上面的整数族——tinyint 语义本就是数）。
    if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
        return Ok(v.map(Value::Bool).unwrap_or(Value::Null));
    }
    // 文本。
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return Ok(v.map(Value::Text).unwrap_or(Value::Null));
    }
    // ⚠ BLOB 显式报错：`Value` 没有 Blob 变体（十一变体对 CalcJson.typeName 封闭，
    // value.rs 模块文档），硬塞 Text 是失真——如实点名带账，等 Value 契约扩展再接。
    Err(DsError::UnsupportedColumnType {
        col: String::new(),
        type_name: row
            .try_get_raw(idx)
            .map(|t| t.type_info().name().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use Value::{Double, Long, Null, Text};

    /// sqlite 内存库全链路冒烟（M3-03 验收的 sqlite 分支）：建表 → 插入 →
    /// SELECT 物化 → 类型逐格断言；NULL 列落 `Value::Null`。
    ///
    /// ⚠ 实测（2026-10-15）：sqlite **声明 `BOOLEAN` 的列在 Any 层取不出来**
    /// （fetch 期即报 `AnyDriverError("Any driver does not support the SQLite
    /// type SqliteTypeInfo(Bool)")`——eager 报错，级联 try_get 根本没机会）。
    /// 故布尔列在 sqlite 上用 INTEGER 0/1（Long）承载；PG 的 BOOL 走 `Bool`
    /// 分支，待真库冒烟验证（带账，RUNLOG §28）。这与蓝图方言表
    /// 「布尔字面量差异」同源：布尔语义本就按库种分叉。
    #[tokio::test]
    async fn sqlite_round_trip_materializes_typed_rows() {
        let src = SqlSource::connect("sqlite::memory:").await.unwrap();
        src.fetch_sql(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, region TEXT, qty INTEGER, price REAL, ok INTEGER)",
        )
        .await
        .unwrap();
        src.fetch_sql("INSERT INTO t VALUES (1, 'North', 100, 12.5, 1), (2, 'East', 200, NULL, 0)")
            .await
            .unwrap();
        let rs = src
            .fetch_sql("SELECT id, region, qty, price, ok FROM t ORDER BY id")
            .await
            .unwrap();
        assert_eq!(rs.cols, ["id", "region", "qty", "price", "ok"]);
        assert_eq!(
            rs.rows[0],
            vec![
                Long(1),
                Text("North".into()),
                Long(100),
                Double(12.5),
                Long(1)
            ]
        );
        // NULL REAL → Null（不是 0.0）；布尔列（INTEGER 0/1）收 Long——见上方实测注。
        assert_eq!(
            rs.rows[1],
            vec![Long(2), Text("East".into()), Long(200), Null, Long(0)]
        );
    }

    /// 空结果集仍保留列名；不能因为 `rows.first()` 为 None 就把 `cols` 清空。
    #[tokio::test]
    async fn empty_result_keeps_columns() {
        let src = SqlSource::connect("sqlite::memory:").await.unwrap();
        src.fetch_sql("CREATE TABLE empty_t (id INTEGER, region TEXT)")
            .await
            .unwrap();
        let rs = src
            .fetch_sql("SELECT id, region FROM empty_t WHERE 0")
            .await
            .unwrap();
        assert_eq!(rs.cols, ["id", "region"]);
        assert!(rs.rows.is_empty());
    }

    /// 换库只换 URL 的反面：未知 scheme 显式报错（不猜、不静默）。
    #[tokio::test]
    async fn unknown_scheme_is_a_loud_error() {
        let rejected = matches!(
            SqlSource::connect("oracle://h/db").await,
            Err(DsError::UnsupportedScheme(_))
        );
        assert!(rejected);
    }

    /// 模板 SQL 原样透传（§8 SQL 注入面口径）：坏 SQL 把 sqlx 错误如实上抛。
    #[tokio::test]
    async fn broken_sql_surfaces_the_driver_error() {
        let src = SqlSource::connect("sqlite::memory:").await.unwrap();
        let err = src.fetch_sql("SELETC 1").await.unwrap_err();
        assert!(matches!(err, DsError::Sql(_)), "{err}");
    }

    /// 钉子（spec-m3 §8 账项「sqlite BOOLEAN 列」）：sqlite 上**声明** `BOOLEAN`
    /// 的列在 Any 层 fetch 期 eager 报错——级联 `try_get` 没机会救。契约 =
    /// **响亮失败**（`DsError::Sql` 且文本点名 "does not support"），绝不静默
    /// 塞占位值；业务侧承载方式 = INTEGER 0/1（见 `sqlite_round_trip` 的 `ok` 列）。
    #[tokio::test]
    async fn sqlite_declared_boolean_column_fails_loudly() {
        let src = SqlSource::connect("sqlite::memory:").await.unwrap();
        src.fetch_sql("CREATE TABLE b (id INTEGER, flag BOOLEAN)")
            .await
            .unwrap();
        src.fetch_sql("INSERT INTO b VALUES (1, 1)").await.unwrap();
        let err = src.fetch_sql("SELECT flag FROM b").await.unwrap_err();
        match err {
            DsError::Sql(e) => assert!(
                e.to_string().contains("does not support"),
                "错误文本应点名不支持该类型，实得：{e}"
            ),
            other => panic!("BOOLEAN 列应报 Sql 级响亮错误，实得 {other}"),
        }
    }
}
