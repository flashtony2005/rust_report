//! SQL 方言差异表（蓝图 §七 7.2 的落地骨架）。
//!
//! 本期**只立表不实现改写**：报表 SQL 是模板自带的用户资产，M3-03 验收 =
//! 原样透传 AnyPool（换库只换 URL）。方言表先行冻结的是「未来做分页 SQL 生成 /
//! SQL 改写时**必须**处理的差异面」，并给类型映射（`super::value_map`）当注释锚。
//!
//! ⚠ 差异点全部转引自蓝图 §7.2（来源为项目调研记录，非本仓探针实证）；
//! 命中实现时须按纪律补真机夹具。

/// 方言记录：一种 URL scheme 一条。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dialect {
    /// URL scheme（`postgres` / `mysql` / `sqlite` / `mssql`）。
    pub scheme: &'static str,
    /// 占位符形态（`?` = 问号序号均可；`$n`；`@p1`）。
    pub placeholder: Placeholder,
    /// 分页子句形态。
    pub paging: Paging,
    /// 布尔字面量（SQL 文本里怎么写真值）。
    pub bool_literal: BoolLiteral,
}

/// 占位符差异（蓝图 §7.2 表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placeholder {
    /// `?` 或 `?1`（MySQL / SQLite）。
    Question,
    /// `$1`（PostgreSQL）。
    Dollar,
    /// `@p1`（MSSQL）。
    At,
}

/// 分页子句差异。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paging {
    /// `LIMIT n OFFSET m`（PG/MySQL/SQLite）。
    LimitOffset,
    /// `OFFSET m ROWS FETCH NEXT n ROWS ONLY`（MSSQL）。
    Fetch,
}

/// 布尔字面量差异。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolLiteral {
    /// `TRUE`/`FALSE` 关键字（PG）。
    Keyword,
    /// `1`/`0`（MySQL tinyint / SQLite 整数 / MSSQL bit）。
    Numeric,
}

/// 已登记方言表。新增一种库 = 加一条记录 + 一个 sqlx 驱动 feature（蓝图口径）。
pub const DIALECTS: &[Dialect] = &[
    Dialect {
        scheme: "postgres",
        placeholder: Placeholder::Dollar,
        paging: Paging::LimitOffset,
        bool_literal: BoolLiteral::Keyword,
    },
    Dialect {
        scheme: "mysql",
        placeholder: Placeholder::Question,
        paging: Paging::LimitOffset,
        bool_literal: BoolLiteral::Numeric,
    },
    Dialect {
        scheme: "sqlite",
        placeholder: Placeholder::Question,
        paging: Paging::LimitOffset,
        bool_literal: BoolLiteral::Numeric,
    },
    Dialect {
        scheme: "mssql",
        placeholder: Placeholder::At,
        paging: Paging::Fetch,
        bool_literal: BoolLiteral::Numeric,
    },
];

/// URL scheme → 方言（未知 scheme 返回 `None`——上层显式报错，不猜）。
///
/// scheme = 首个 `:` 之前：既覆盖 `sqlite://file.db`，也覆盖
/// `sqlite::memory:`（sqlx 的内存库 URL 没有 `//`，按 `://` 切会把
/// 整串当 scheme——2026-10-15 冒烟实测）。
pub fn for_url(url: &str) -> Option<&'static Dialect> {
    let scheme = url.split(':').next()?;
    DIALECTS.iter().find(|d| d.scheme == scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemes_map_to_their_dialect() {
        assert_eq!(
            for_url("postgres://u:p@h/db").unwrap().placeholder,
            Placeholder::Dollar
        );
        assert_eq!(
            for_url("mysql://u:p@h/db").unwrap().bool_literal,
            BoolLiteral::Numeric
        );
        assert_eq!(
            for_url("sqlite://data.db").unwrap().paging,
            Paging::LimitOffset
        );
        assert_eq!(for_url("mssql://h/db").unwrap().paging, Paging::Fetch);
    }

    #[test]
    fn unknown_scheme_is_none_not_a_guess() {
        assert!(for_url("oracle://h/db").is_none());
        assert!(for_url("no-scheme").is_none());
    }
}
