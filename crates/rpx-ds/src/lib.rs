//! rpx-ds：数据源层（M3-03）。
//!
//! 职责：把模板声明的数据集配置（`rpx_core::dataset::FileDataSetConfig` 的 SQL/文件
//! 同族）取数并**物化**为 [`rpx_core::rowset::RowSet`]——引擎求值态的唯一输入
//! （`rpx_engine::expr::ds::DataSet::from_rowset`，定案 `docs/spec-m3.md` §3.4）。
//!
//! 本 crate **不依赖** `rpx-engine`（供数方不依赖消费方，§3.4 理由 1）。

pub mod dialect;
pub mod file_source;
pub mod sql_source;

pub use sql_source::{DsError, SqlSource};

/// 数据源统一入口（蓝图 §七 的 `trait DataSource`）。
///
/// Rust 侧用 trait 对象等价 Java 的 `Class.forName(factoryClass)` 插件机制：
/// 引擎不认识任何具体数据源类型，只认这个契约。`query` 的含义随实现：
/// `SqlSource` 收 SQL 文本，文件源收文件名（蓝图 7.2 的 File/Builtin 各走各实现）。
///
/// 模板自带的查询文本是**用户资产**（单实例可信输入——`spec-m3.md` §8
/// SQL 注入面同口径），各实现**原样透传**。
pub trait DataSource {
    /// 按查询语义取数并物化。
    fn fetch(
        &self,
        query: &str,
    ) -> impl std::future::Future<Output = Result<rpx_core::rowset::RowSet, DsError>> + Send;
}

impl DataSource for SqlSource {
    fn fetch(
        &self,
        query: &str,
    ) -> impl std::future::Future<Output = Result<rpx_core::rowset::RowSet, DsError>> + Send {
        self.fetch_sql(query)
    }
}
