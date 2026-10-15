//! PG 真连冒烟（`spec-m3.md` §8 账项「PG/MySQL 真连冒烟」的 PG 半边）。
//!
//! # 门控协议
//!
//! 默认跳过（打印 SKIP 即通过）：G3 `cargo test --workspace` 在无 PG 服务器的
//! 沙箱里必须保持全绿。设 `RPX_PG_URL` 环境变量后才真跑：
//!
//! ```text
//! RPX_PG_URL=postgres://user:pass@host:port/db cargo test -p rpx-ds --test smoke_pg
//! ```
//!
//! 首次真机（2026-10-15）：公司服务器 192.168.1.77:54323，库 `test`，用户 `system`，
//! `SELECT version()` 自报 **KingbaseES V008R006C008B0014**（人大金仓，PG 线协议
//! 兼容，sqlx postgres 驱动直连成功）——「PG 真连冒烟」在金仓上实跑成立。
//! 服务器文本只打进日志不断言；断言依赖 Any 层映射契约：
//! **整数族统一 `Long`**（sqlx-core-0.9 `i64::compatible = is_integer`，
//! int.rs:15/43/71——级联先试 i64，int2/int4/int8 全兼容，金仓真机同此）、
//! REAL→`Float`（f64 与 float kind 不兼容的级联结果）、BOOL→`Bool`、NULL→`Null`。
//!
//! # 带账清偿点
//!
//! ① `sql_source.rs` 实测注「PG 的 BOOL 走 `Bool` 分支待真库冒烟验证」——
//!    本测试 `ok BOOL` 列钉死该分支（金仓真机 ✅）；
//! ② `NUMERIC` 列**响亮失败**契约（`any_cell` 尾注「不塞占位值」）——
//!    金仓真机实测：Any 层 fetch 期即 eager 拒绝，形态为 `DsError::Sql`
//!    （ColumnDecode 点名 "does not support ... Numeric"），到不了级联。

use rpx_core::value::Value::{Bool, Double, Float, Long, Null, Text};
use rpx_ds::SqlSource;

/// 环境未给 PG 时静默跳过（G3 全绿前提）；给了才真连。
fn pg_url() -> Option<String> {
    std::env::var("RPX_PG_URL").ok().filter(|s| !s.is_empty())
}

/// 全链路：真连 → 服务器自报 → 建表 → 插入 → 类型逐格物化 → 清理。
///
/// 用**真实表**而非 TEMP 表：`AnyPool` 非网络库按 10 连接池复用，TEMP 表
/// 绑定单连接，跨池连接不可见（`sql_source.rs::connect` 坑③注释）。
/// 表名带 `rpx_smoke_` 前缀并在结尾 DROP——`test` 库内的临时验证残留为空。
#[tokio::test]
async fn pg_round_trip_materializes_typed_rows() {
    let Some(url) = pg_url() else {
        eprintln!("[smoke_pg] SKIP：未设 RPX_PG_URL（沙箱无 PG 服务器时的默认路径）");
        return;
    };

    // ① 真连：postgres:// 前缀走方言表 → Any postgres 驱动。
    let src = SqlSource::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("[smoke_pg] 连接失败 {url}：{e}"));

    // ② 服务器自报（KingbaseES / PostgreSQL 靠这段文本辨认，只打印不断言——
    //    断言依赖的是下方逐类型列的 Any 层映射，与发行版无关）。
    let ver = src.fetch_sql("SELECT version()").await.unwrap();
    eprintln!("[smoke_pg] server = {:?}", ver.rows[0][0]);

    // ③ 建表（先删后建，幂等）：一张表覆盖 any_cell 全部已实现分支 + 两个带账点。
    src.fetch_sql("DROP TABLE IF EXISTS rpx_smoke_t")
        .await
        .unwrap();
    src.fetch_sql(
        "CREATE TABLE rpx_smoke_t (\
            id BIGINT, qty INTEGER, small SMALLINT, price DOUBLE PRECISION, \
            ratio REAL, region VARCHAR(20), ok BOOL, note TEXT, missing INTEGER, \
            amount NUMERIC(12,2))",
    )
    .await
    .unwrap();

    // ④ 插入：覆盖 NULL / 负数 / 小数 / 布尔两态。
    src.fetch_sql(
        "INSERT INTO rpx_smoke_t VALUES \
         (1, 100, 7, 12.5, 0.25, '华北', TRUE, '中文值', NULL, 1234.56), \
         (2, -200, -9, -0.5, 2.0, 'East', FALSE, NULL, 42, 0.01)",
    )
    .await
    .unwrap();

    // ⑤ 物化 + 类型逐格断言（列序对齐 INSERT）。
    let rs = src
        .fetch_sql(
            "SELECT id, qty, small, price, ratio, region, ok, note, missing \
             FROM rpx_smoke_t ORDER BY id",
        )
        .await
        .unwrap();
    eprintln!("[smoke_pg] cols = {:?}", rs.cols);
    assert_eq!(
        rs.cols,
        ["id", "qty", "small", "price", "ratio", "region", "ok", "note", "missing"]
    );
    // 行 1：整数族**统一 Long**（sqlx Any 层 `i64::compatible = is_integer`，
    //       sqlx-core-0.9 int.rs:15/43/71——级联先试 i64，int2/int4/int8 全兼容；
    //       金仓真机 2026-10-15 实测同此）、DOUBLE→Double、REAL→Float、
    //       VARCHAR/TEXT→Text、BOOL→Bool（带账清偿点①）、NULL→Null。
    assert_eq!(
        rs.rows[0],
        vec![
            Long(1),
            Long(100),
            Long(7),
            Double(12.5),
            Float(0.25),
            Text("华北".into()),
            Bool(true),
            Text("中文值".into()),
            Null,
        ]
    );
    // 行 2：负数与布尔 false 同样保真。
    assert_eq!(
        rs.rows[1],
        vec![
            Long(2),
            Long(-200),
            Long(-9),
            Double(-0.5),
            Float(2.0),
            Text("East".into()),
            Bool(false),
            Null,
            Long(42),
        ]
    );

    // ⑥ 带账清偿点②：NUMERIC 列**响亮失败**（不是静默占位）。金仓真机实测：
    //    Any 层对 Numeric 类型 fetch 期即 eager 拒绝（与 sqlite BOOLEAN 同构），
    //    错误形态 = `DsError::Sql(ColumnDecode("...Any driver does not support
    //    the Postgres type ... Numeric"))`——根本到不了 `any_cell` 的级联。
    //    断言钉「响亮失败 + 文本点名不支持」，错误文本细节不钉死（驱动可改名）。
    let numeric_err = src
        .fetch_sql("SELECT amount FROM rpx_smoke_t ORDER BY id")
        .await
        .unwrap_err();
    match numeric_err {
        rpx_ds::DsError::Sql(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("does not support") && msg.contains("Numeric"),
                "错误应点名 Any 层不支持 Numeric，实得：{msg}"
            );
            eprintln!("[smoke_pg] NUMERIC 列 fetch 期响亮失败（带账②实锤）：{msg}");
        }
        other => panic!("[smoke_pg] NUMERIC 应报 Sql 级响亮错误，实得 {other}"),
    }

    // ⑦ 清理：先删后插纪律的镜像——验证完表不留痕。
    src.fetch_sql("DROP TABLE rpx_smoke_t").await.unwrap();
    eprintln!("[smoke_pg] OK：类型物化 + 两个带账点全部实锤");
}
