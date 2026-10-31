//! SQL 数据源：**原生驱动池**取行 → [`rpx_core::rowset::RowSet`] 物化。
//!
//! # 链路（蓝图 §7.2 + `spec-m3.md` §3.4）
//!
//! ```text
//! url（postgres:// / mysql:// / sqlite://）
//!   → 方言表定驱动 → PgPool / MySqlPool / SqlitePool（**原生**，不走 Any）
//!   → sqlx::query(sql).fetch(&pool)   // 模板 SQL 原样透传
//!   → 逐行逐列 cell → Value（按方言级联 try_get）
//!   → RowSet { name, cols = 结果集列名, rows }
//! ```
//!
//! # 2026-10-23：为什么弃用 `AnyPool`（用户真机实锤）
//!
//! sqlx 0.9 的 PG→Any 类型桥**只认** Bool/Void/Int2/Int4/Int8/Float4/Float8/
//! Bytea/Text/Varchar/citext（`sqlx-postgres-0.9.0/src/any.rs` 的
//! `TryFrom<&PgTypeInfo> for AnyTypeInfo`，`_ =>` 分支直接报错）。业务表里的
//! `TIMESTAMP` / `NUMERIC` / `DATE` 全不在表内，fetch 解码期即 eager 报
//! `ColumnDecode { source: AnyDriverError("Any driver does not support the
//! Postgres type PgTypeInfo(Timestamp)") }`——用户连真库的第一张表就撞上。
//! AnyPool 对报表是**死路**：这类列是常态，没有「绕」的余地。
//!
//! ⇒ 按 scheme 直接建原生池，逐方言级联解码。类型映射收敛成：
//! 整数族 → `Long`（延续 Any 时代的口径，`smoke_pg.rs` 金仓实测同此）、
//! `NUMERIC/DECIMAL` → `Value::Decimal`（规范十进制字面量，对应
//! `java.math.BigDecimal`——**带账②的清偿点**：原账「NUMERIC 响亮失败」就此作废，
//! `smoke_pg.rs` 的断言已随之改写）、日期/时间 → `Text`（`Value` 没有日期变体，
//! 按显示格式落字符串；TIMESTAMPTZ 的时区信息折进字符串，渲染层不感知）、
//! `String` → `Text`、`BOOL` → `Bool`。
//!
//! # 两个仍然成立的坑（蓝图 §7.2）
//!
//! ① 报表 SQL 是用户模板带的，不受我们控制，`AssertSqlSafe` 显式豁免
//!    `SqlSafeStr` lint（注入面口径见 `spec-m3.md` §8）；
//! ② SQLite 同一时刻只允许一个写者 ⇒ 池上限按库种配置（sqlite=1，网络库 10+）。

use futures_util::TryStreamExt;
use rpx_core::rowset::RowSet;
use rpx_core::value::Value;
// `prepare` 在 `Executor` 上（`Connection` 没有）；`Row` 给 `try_get`。
use sqlx::{Column, Executor, Row, SqlSafeStr, Statement};

/// 默认最多物化 100k 行；超过即显式失败，避免一次性打爆内存。
pub const DEFAULT_MAX_ROWS: usize = 100_000;

/// 取数错误。具体到可定位（渲染期报错兜底是蓝图口径，错误文本就是给用户的）。
#[derive(Debug)]
pub enum DsError {
    /// URL scheme 不在方言表（`dialect::for_url` 为 `None`）——显式报错不猜。
    UnsupportedScheme(String),
    /// 连接/查询失败（sqlx 原样透传）。
    Sql(sqlx::Error),
    /// 某列的值类型解不出（骨架期未覆盖的列型，如实点名而非塞占位值）。
    UnsupportedColumnType {
        /// 列名。
        col: String,
        /// 驱动报的类型名（如 `TIMESTAMP`）。
        type_name: String,
    },
    /// 结果行数超过上限，拒绝继续物化。
    TooManyRows { limit: usize },
}

impl std::fmt::Display for DsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DsError::UnsupportedScheme(url) => write!(f, "不支持的 URL scheme：{url}"),
            DsError::Sql(e) => write!(f, "SQL 取数失败：{e}"),
            DsError::UnsupportedColumnType { col, type_name } => {
                write!(f, "列 {col} 的类型 {type_name} 暂不支持（未覆盖的列型）")
            }
            DsError::TooManyRows { limit } => write!(
                f,
                "SQL 结果超过 {limit} 行上限；请缩小查询或提高 RPX_SQL_MAX_ROWS"
            ),
        }
    }
}

impl std::error::Error for DsError {}

impl From<sqlx::Error> for DsError {
    fn from(e: sqlx::Error) -> Self {
        DsError::Sql(e)
    }
}

impl DsError {
    /// 把「类型不支持」补上列名（调用处按列迭代，错误在这里只带了类型名）。
    fn with_col(self, col: String) -> Self {
        match self {
            DsError::UnsupportedColumnType { type_name, .. } => {
                DsError::UnsupportedColumnType { col, type_name }
            }
            other => other,
        }
    }
}

/// 按参数列表逐个绑定。宏而不是泛型：`bind` 的返回类型绑定在具体 `Database` 上，
/// 泛型里写不出来 —— 而三方言的绑定循环必须逐字一致（否则某个方言漏一个 `?` 就错位）。
///
/// ⚠ 类型对齐：JSON 的整数走 `i64`、浮点走 `f64`、其余按文本。PG 上 `date >= $1`
/// 收到 TEXT 会直接报「operator does not exist」——所以**日期不走参数**，
/// 编译期写成校验过的字面量（见 `rpx_semantic::query::sql` 模块头）。
macro_rules! bound {
    ($pool:expr, $sql:expr, $params:expr) => {{
        let mut q = sqlx::query(sqlx::AssertSqlSafe($sql.to_owned()));
        for v in $params {
            q = match v {
                Value::Null => q.bind(Option::<String>::None),
                Value::Text(s) => q.bind(s.clone()),
                Value::Int(i) => q.bind(i64::from(*i)),
                Value::Long(l) => q.bind(*l),
                Value::Short(s) => q.bind(i64::from(*s)),
                Value::Byte(b) => q.bind(i64::from(*b)),
                Value::Float(f) => q.bind(f64::from(*f)),
                Value::Double(d) => q.bind(*d),
                // Decimal 在 rpx-core 里存的是规范字面量，这里落成 f64 绑定
                Value::Decimal(text) => q.bind(text.parse::<f64>().unwrap_or(0.0)),
                Value::Bool(b) => q.bind(*b),
                Value::List(_) => q.bind(Option::<String>::None),
            };
        }
        q
    }};
}

/// 按方言持有**原生**连接池（Any 层会丢类型，见模块头——这里必须原生）。
enum Pool {
    Pg(sqlx::postgres::PgPool),
    MySql(sqlx::mysql::MySqlPool),
    Sqlite(sqlx::sqlite::SqlitePool),
}

/// SQL 数据源。连接串即身份——同 URL 复用池（池注册表在 server 编排层做，
/// 本 crate 保持无状态：一个 `SqlSource` 对应一个池）。
pub struct SqlSource {
    pool: Pool,
}

impl SqlSource {
    /// 建池并连接。sqlite 系 URL 强制 `max_connections = 1`（坑②）。
    pub async fn connect(url: &str) -> Result<Self, DsError> {
        let driver = crate::dialect::for_url(url)
            .ok_or_else(|| DsError::UnsupportedScheme(url.to_string()))?;
        let pool = match driver.scheme {
            "postgres" => Pool::Pg(
                sqlx::postgres::PgPoolOptions::new()
                    .max_connections(10)
                    .connect(url)
                    .await?,
            ),
            "mysql" => Pool::MySql(
                sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(10)
                    .connect(url)
                    .await?,
            ),
            "sqlite" => Pool::Sqlite(
                sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect(url)
                    .await?,
            ),
            other => return Err(DsError::UnsupportedScheme(other.to_string())),
        };
        Ok(SqlSource { pool })
    }

    /// 执行非查询语句（DDL/DML），返回受影响行数。
    ///
    /// 测试基建用（`sqlite_fixture.rs` 内存库自建表，不依赖外部服务器）；也是
    /// 未来 CLI 初始化数据库的出口。SQL 安全性与 [`fetch_sql`](Self::fetch_sql)
    /// 同规（模板侧由服务端把关，此处不二次过滤）。
    pub async fn execute(&self, sql: &str) -> Result<u64, DsError> {
        let n = match &self.pool {
            Pool::Pg(p) => sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
                .execute(p)
                .await?
                .rows_affected(),
            Pool::MySql(p) => sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
                .execute(p)
                .await?
                .rows_affected(),
            Pool::Sqlite(p) => sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
                .execute(p)
                .await?
                .rows_affected(),
        };
        Ok(n)
    }

    /// 默认最大结果行数；可用 `RPX_SQL_MAX_ROWS` 覆盖。
    fn default_max_rows() -> usize {
        std::env::var("RPX_SQL_MAX_ROWS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_MAX_ROWS)
    }

    /// 执行模板 SQL 并物化。列名取结果集表头（SQL 数据集的列 = 查询产物，
    /// 与产品 SQL 数据集语义一致），行值经各方言 `*_cell` 收敛到 `Value`。
    pub async fn fetch_sql(&self, sql: &str) -> Result<RowSet, DsError> {
        self.fetch_sql_limited(sql, Self::default_max_rows()).await
    }

    /// 带显式行数上限的 [`fetch_sql`](Self::fetch_sql)；超限返回 `TooManyRows`。
    ///
    /// 「超限」是**错误**不是截断：报表少几行比多几行危险得多（数字会悄悄变小）。
    /// 只要样本的场景用 [`fetch_sql_take`](Self::fetch_sql_take)。
    pub async fn fetch_sql_limited(&self, sql: &str, max_rows: usize) -> Result<RowSet, DsError> {
        if max_rows == 0 {
            return Err(DsError::TooManyRows { limit: 0 });
        }
        self.collect(sql, &[], max_rows, OnFull::Fail).await
    }

    /// 取前 `take` 行，**到量即停**（不报错）。
    ///
    /// 与 [`fetch_sql_limited`](Self::fetch_sql_limited) 是两种语义，别混用：
    /// 那个是"整份数据必须装得下"，这个是"我只要看一眼"（数据源探测、列画像）。
    /// 实现上仍然把 SQL 原样透传，只提前 `break`，不改写用户的 SQL。
    pub async fn fetch_sql_take(&self, sql: &str, take: usize) -> Result<RowSet, DsError> {
        self.collect(sql, &[], take.max(1), OnFull::Stop).await
    }

    /// 带**参数绑定**的取数：`params` 依次绑定到 SQL 里的 `$n` / `?`。
    ///
    /// 语义层的编译器只产出占位符（值一律不拼字符串），所以执行侧必须有这条路；
    /// 行数上限同 [`fetch_sql_limited`]（超限即错）。
    pub async fn fetch_sql_bound(
        &self,
        sql: &str,
        params: &[Value],
        max_rows: usize,
    ) -> Result<RowSet, DsError> {
        if max_rows == 0 {
            return Err(DsError::TooManyRows { limit: 0 });
        }
        self.collect(sql, params, max_rows, OnFull::Fail).await
    }

    /// 取数主体：`on_full` 决定装不下时是报错还是停下；`params` 为空则不绑定。
    async fn collect(
        &self,
        sql: &str,
        params: &[Value],
        max_rows: usize,
        on_full: OnFull,
    ) -> Result<RowSet, DsError> {
        let (cols, rows) = match &self.pool {
            Pool::Pg(p) => {
                let cols = pg_columns(p, sql).await?;
                let mut stream = bound!(p, sql, params).fetch(p);
                let mut rows: Vec<Vec<Value>> = Vec::new();
                while let Some(row) = stream.try_next().await? {
                    if full(&rows, max_rows, on_full)? {
                        break;
                    }
                    let mut vals = Vec::with_capacity(cols.len());
                    for (i, col) in cols.iter().enumerate() {
                        vals.push(pg_cell(&row, i).map_err(|e| e.with_col(col.clone()))?);
                    }
                    rows.push(vals);
                }
                (cols, rows)
            }
            Pool::MySql(p) => {
                let cols = mysql_columns(p, sql).await?;
                let mut stream = bound!(p, sql, params).fetch(p);
                let mut rows: Vec<Vec<Value>> = Vec::new();
                while let Some(row) = stream.try_next().await? {
                    if full(&rows, max_rows, on_full)? {
                        break;
                    }
                    let mut vals = Vec::with_capacity(cols.len());
                    for (i, col) in cols.iter().enumerate() {
                        vals.push(mysql_cell(&row, i).map_err(|e| e.with_col(col.clone()))?);
                    }
                    rows.push(vals);
                }
                (cols, rows)
            }
            Pool::Sqlite(p) => {
                let cols = sqlite_columns(p, sql).await?;
                let mut stream = bound!(p, sql, params).fetch(p);
                let mut rows: Vec<Vec<Value>> = Vec::new();
                while let Some(row) = stream.try_next().await? {
                    if full(&rows, max_rows, on_full)? {
                        break;
                    }
                    let mut vals = Vec::with_capacity(cols.len());
                    for (i, col) in cols.iter().enumerate() {
                        vals.push(sqlite_cell(&row, i).map_err(|e| e.with_col(col.clone()))?);
                    }
                    rows.push(vals);
                }
                (cols, rows)
            }
        };
        // name 由调用方（数据集配置）定；SQL 层只有行集。此处以 SQL 首词占位，
        // 由 server 编排层用配置名覆盖——见模块文档的职责边界。
        Ok(RowSet {
            name: String::new(),
            cols,
            rows,
        })
    }
}

/// 装满时的行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnFull {
    /// 报错（`fetch_sql_limited`：数据必须完整）。
    Fail,
    /// 停下（`fetch_sql_take`：只要样本）。
    Stop,
}

/// 返回 `true` 表示调用方应当 `break`（仅在 [`OnFull::Stop`] 时可能为真）。
fn full(rows: &[Vec<Value>], max_rows: usize, on_full: OnFull) -> Result<bool, DsError> {
    if rows.len() < max_rows {
        return Ok(false);
    }
    match on_full {
        OnFull::Fail => Err(DsError::TooManyRows { limit: max_rows }),
        OnFull::Stop => Ok(true),
    }
}

// ---------------------------------------------------------------------------
// 列名：`fetch` 在空结果时拿不到列元数据 ⇒ 单独 prepare 一次。
// ---------------------------------------------------------------------------

async fn pg_columns(pool: &sqlx::postgres::PgPool, sql: &str) -> Result<Vec<String>, DsError> {
    let mut conn = pool.acquire().await?;
    let st = conn
        .prepare(sqlx::AssertSqlSafe(sql.to_owned()).into_sql_str())
        .await?;
    Ok(st.columns().iter().map(|c| c.name().to_string()).collect())
}

async fn mysql_columns(pool: &sqlx::mysql::MySqlPool, sql: &str) -> Result<Vec<String>, DsError> {
    let mut conn = pool.acquire().await?;
    let st = conn
        .prepare(sqlx::AssertSqlSafe(sql.to_owned()).into_sql_str())
        .await?;
    Ok(st.columns().iter().map(|c| c.name().to_string()).collect())
}

async fn sqlite_columns(
    pool: &sqlx::sqlite::SqlitePool,
    sql: &str,
) -> Result<Vec<String>, DsError> {
    let mut conn = pool.acquire().await?;
    let st = conn
        .prepare(sqlx::AssertSqlSafe(sql.to_owned()).into_sql_str())
        .await?;
    Ok(st.columns().iter().map(|c| c.name().to_string()).collect())
}

// ---------------------------------------------------------------------------
// 逐格解码（级联 try_get：类型不匹配 -> Err 换下一型，全落空按类型名报错）
// ---------------------------------------------------------------------------

/// 驱动侧类型名（`UnsupportedColumnType` 的点名依据）。
///
/// 用宏而不是泛型：`try_get_raw` 的返回类型绑定在具体 Database 上，泛型里
/// `type_info()` 的 trait 约束写出来反而更绕。
macro_rules! raw_type_name {
    ($row:expr, $idx:expr) => {{
        use sqlx::{TypeInfo, ValueRef};
        $row.try_get_raw($idx)
            .map(|v| v.type_info().name().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    }};
}

/// PG 一格。级联顺序 = 按业务表的出现概率排：整数 -> 浮点 -> 定点 -> 日期时间 -> 文本。
fn pg_cell(row: &sqlx::postgres::PgRow, idx: usize) -> Result<Value, DsError> {
    // 整数族**统一 Long**（延续 Any 时代的口径：报告侧不区分 int 宽度；
    // sqlx PG 里 i64 只兼容 INT8、i32 只兼容 INT4、i16 只兼容 INT2，逐个试）。
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return Ok(v.map(Value::Long).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<i32>, _>(idx) {
        return Ok(v.map(|x| Value::Long(i64::from(x))).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<i16>, _>(idx) {
        return Ok(v.map(|x| Value::Long(i64::from(x))).unwrap_or(Value::Null));
    }
    // 浮点族：DOUBLE 优先，REAL -> f32（Value 保留 Float 变体，不归一——M2-14 契约）。
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return Ok(v.map(Value::Double).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<f32>, _>(idx) {
        return Ok(v.map(Value::Float).unwrap_or(Value::Null));
    }
    // 定点：NUMERIC/DECIMAL -> 规范十进制字面量（带账②清偿点，见模块头）。
    if let Ok(v) = row.try_get::<Option<sqlx::types::Decimal>, _>(idx) {
        return Ok(v
            .map(|d| Value::Decimal(d.to_string()))
            .unwrap_or(Value::Null));
    }
    // 日期/时间 -> 显示字符串。TIMESTAMP 走 NaiveDateTime、TIMESTAMPTZ 走
    // DateTime<Utc>（两者互不兼容，级联各试一次）。Value 没有日期变体，
    // TIMESTAMPTZ 的时区折进字符串不外露。
    if let Ok(v) = row.try_get::<Option<sqlx::types::chrono::NaiveDateTime>, _>(idx) {
        return Ok(v
            .map(|d| Value::Text(d.format("%Y-%m-%d %H:%M:%S%.f").to_string()))
            .unwrap_or(Value::Null));
    }
    if let Ok(v) =
        row.try_get::<Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>, _>(idx)
    {
        return Ok(v
            .map(|d| Value::Text(d.format("%Y-%m-%d %H:%M:%S%.f").to_string()))
            .unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<sqlx::types::chrono::NaiveDate>, _>(idx) {
        return Ok(v
            .map(|d| Value::Text(d.format("%Y-%m-%d").to_string()))
            .unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<sqlx::types::chrono::NaiveTime>, _>(idx) {
        return Ok(v
            .map(|d| Value::Text(d.format("%H:%M:%S%.f").to_string()))
            .unwrap_or(Value::Null));
    }
    // 布尔（PG BOOL；带账①已在金仓真机实锤）。
    if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
        return Ok(v.map(Value::Bool).unwrap_or(Value::Null));
    }
    // 文本（TEXT / VARCHAR / CHAR / NAME / UNKNOWN）。
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return Ok(v.map(Value::Text).unwrap_or(Value::Null));
    }
    Err(DsError::UnsupportedColumnType {
        col: String::new(),
        type_name: raw_type_name!(row, idx),
    })
}

/// MySQL 一格。顺序同 PG；TINYINT(1) 走整数族（tinyint 语义本就是数）。
fn mysql_cell(row: &sqlx::mysql::MySqlRow, idx: usize) -> Result<Value, DsError> {
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return Ok(v.map(Value::Long).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<i32>, _>(idx) {
        return Ok(v.map(|x| Value::Long(i64::from(x))).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<i16>, _>(idx) {
        return Ok(v.map(|x| Value::Long(i64::from(x))).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<u64>, _>(idx) {
        // BIGINT UNSIGNED（COUNT(*) 常客）。
        return Ok(v
            .map(|x| Value::Long(i64::try_from(x).unwrap_or(i64::MAX)))
            .unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return Ok(v.map(Value::Double).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<f32>, _>(idx) {
        return Ok(v.map(Value::Float).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<sqlx::types::Decimal>, _>(idx) {
        return Ok(v
            .map(|d| Value::Decimal(d.to_string()))
            .unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<sqlx::types::chrono::NaiveDateTime>, _>(idx) {
        return Ok(v
            .map(|d| Value::Text(d.format("%Y-%m-%d %H:%M:%S%.f").to_string()))
            .unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<sqlx::types::chrono::NaiveDate>, _>(idx) {
        return Ok(v
            .map(|d| Value::Text(d.format("%Y-%m-%d").to_string()))
            .unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<sqlx::types::chrono::NaiveTime>, _>(idx) {
        return Ok(v
            .map(|d| Value::Text(d.format("%H:%M:%S%.f").to_string()))
            .unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
        return Ok(v.map(Value::Bool).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return Ok(v.map(Value::Text).unwrap_or(Value::Null));
    }
    Err(DsError::UnsupportedColumnType {
        col: String::new(),
        type_name: raw_type_name!(row, idx),
    })
}

/// SQLite 一格：动态类型，只有 Long / Double / Text / BLOB 四种宿主。
fn sqlite_cell(row: &sqlx::sqlite::SqliteRow, idx: usize) -> Result<Value, DsError> {
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return Ok(v.map(Value::Long).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return Ok(v.map(Value::Double).unwrap_or(Value::Null));
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return Ok(v.map(Value::Text).unwrap_or(Value::Null));
    }
    Err(DsError::UnsupportedColumnType {
        col: String::new(),
        type_name: raw_type_name!(row, idx),
    })
}
