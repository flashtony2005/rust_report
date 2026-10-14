//! 报表资产库（M3-04）：`.rpx` BLOB + 元信息 + 参数清单。
//!
//! # 选型偏离定案：sqlx/sqlite 承载，不引 libSQL
//!
//! 蓝图 §7.1 选 libSQL（Turso 本地模式）。本期实际用 **sqlx 的 sqlite 驱动**，
//! 理由（都是硬事实）：
//!
//! 1. `libsql` crate **不在本机离线缓存**（`~/.cargo/registry` 无此包），
//!    沙箱无法下载验证——按「禁止无基准对拍」纪律，引一个跑不起来的依赖是空话；
//! 2. 蓝图自注（§7.1 诚实说明）：只跑本地模式时 libSQL 的增量价值主要是
//!    async API 和 Turso CLI 生态，**文件格式仍与 SQLite 兼容**——sqlx 的
//!    sqlite 驱动给出同样的 async API 与同样的文件格式；
//! 3. sqlx 已因 M3-03 入仓入白名单（`rpx-ds`），零新增依赖面；
//! 4. 升级路径保留：日后换 libSQL 只动本文件（`AssetStore` 的构造与执行层），
//!    调用方（rpx-server 路由）零改动——与蓝图「new_local 换 new_remote_replica
//!    一行」的承诺同一精神。
//!
//! # 职责边界（蓝图 §7.1 开篇）
//!
//! 平台库只存**平台资产**（报表 BLOB / 参数清单 / 数据源配置 / 结果缓存），
//! 业务取数仍走报表自己的数据源（[`super::sql_source`]）——两者互不替代。
//!
//! # schema（蓝图 §8.2 草稿落地，本期只建用到的两张表）
//!
//! `reports`：BLOB 是唯一事实源；`report_params`：参数清单在**上传时**从模板
//! 解析落库（`GET /meta` 不再解模板——读库即答，这也让前端生成参数表单
//! 不依赖解析器）。

use rpx_core::ReportDefine;
use sqlx::Row;

/// 平台库句柄。一个 [`AssetStore`] 对应一个库文件（SQLite 单写者，
/// 池上限固定 1——蓝图坑③同款）。
pub struct AssetStore {
    pool: sqlx::sqlite::SqlitePool,
}

/// 资产库错误。
#[derive(Debug)]
pub enum AssetError {
    /// 模板字节解不开（魔数 / 截断）——上传面直接对用户。
    Parse(rpx_core::RpxError),
    /// 库操作失败（sqlx 原样透传）。
    Sql(sqlx::Error),
    /// 指定 id 的报表不存在。
    NotFound(i64),
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetError::Parse(e) => write!(f, "模板解析失败：{e}"),
            AssetError::Sql(e) => write!(f, "资产库操作失败：{e}"),
            AssetError::NotFound(id) => write!(f, "报表不存在：id={id}"),
        }
    }
}

impl std::error::Error for AssetError {}

impl From<sqlx::Error> for AssetError {
    fn from(e: sqlx::Error) -> Self {
        AssetError::Sql(e)
    }
}

/// 参数清单条目（`GET /meta` 的行；`spec-m3.md` M3-04 验收）。
///
/// 「静态/动态两套 MetaData」= 产品 `Param.paramType` 的分组
/// （`Param.java:22-24`：`PARAM_NORMAL=0` 静态 / `PARAM_DYNAMIC=1` 动态 /
/// `PARAM_ATTRIBUTE=2` 属性）。原样保存 `param_type` 不做归并——
/// `is_static` 只是 `param_type == 0` 的派生，语义不丢。
#[derive(Debug, Clone, PartialEq)]
pub struct AssetParam {
    /// 参数名。
    pub name: String,
    /// 原样 `Param.paramType`（0=静态 1=动态 2=属性）。
    pub param_type: i8,
    /// 静态参数（`param_type == 0`）。
    pub is_static: bool,
    /// 原样 `Param.dataType`（`Types2.DT_*`，前端表单的输入控件类型）。
    pub data_type: i8,
    /// 模板声明的默认值（`Param.value` 槽，字符串原样）。
    pub default_val: Option<String>,
    /// 描述（`Param.description`）。
    pub description: Option<String>,
}

/// 列表条目（`GET /reports`）。
#[derive(Debug, Clone, PartialEq)]
pub struct AssetSummary {
    pub id: i64,
    pub name: String,
    pub version: i64,
    pub created_at: String,
}

impl AssetStore {
    /// 打开（不存在则建表）。`url` 是 sqlite 连接串：
    /// `sqlite::memory:`（测试）/ `sqlite://rpx.db`（部署）。
    pub async fn open(url: &str) -> Result<Self, AssetError> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(url)
            .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS reports(\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\
             name TEXT NOT NULL,\
             rpx BLOB NOT NULL,\
             version INTEGER NOT NULL DEFAULT 1,\
             created_at TEXT NOT NULL DEFAULT (datetime('now')))",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS report_params(\
             report_id INTEGER NOT NULL REFERENCES reports(id),\
             name TEXT NOT NULL,\
             param_type INTEGER NOT NULL,\
             data_type INTEGER NOT NULL,\
             default_val TEXT,\
             description TEXT,\
             PRIMARY KEY (report_id, name))",
        )
        .execute(&pool)
        .await?;
        Ok(AssetStore { pool })
    }

    /// 上传：**魔数校验**（`rpx_core::open_bytes`，坏字节在此挡下）→ 解析
    /// 参数清单 → 写 BLOB 与清单。返回新 id。
    ///
    /// `name` 由调用方给（HTTP 面取上传文件名去扩展名，与 `/render` 的
    /// `report_name` 同口径）。
    pub async fn put(&self, name: &str, rpx: &[u8]) -> Result<i64, AssetError> {
        let rd = rpx_core::open_bytes(rpx).map_err(AssetError::Parse)?;
        let mut tx = self.pool.begin().await?;
        let rec = sqlx::query("INSERT INTO reports(name, rpx) VALUES (?1, ?2)")
            .bind(name)
            .bind(rpx)
            .execute(&mut *tx)
            .await?;
        let id = rec.last_insert_rowid();
        // 清单映射经 [`param_entries`] 单一事实源（与单测同一份逻辑）。
        for p in param_entries(&rd) {
            sqlx::query(
                "INSERT INTO report_params(report_id, name, param_type, data_type, default_val, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(id)
            .bind(p.name)
            .bind(p.param_type)
            .bind(p.data_type)
            .bind(p.default_val)
            .bind(p.description)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    /// 列表（名称/版本/时间；蓝图 §八 路由表口径）。
    pub async fn list(&self) -> Result<Vec<AssetSummary>, AssetError> {
        let rows = sqlx::query("SELECT id, name, version, created_at FROM reports ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| AssetSummary {
                id: r.get::<i64, _>("id"),
                name: r.get::<String, _>("name"),
                version: r.get::<i64, _>("version"),
                created_at: r.get::<String, _>("created_at"),
            })
            .collect())
    }

    /// 参数清单（`GET /reports/{id}/meta`；读库即答，不再解模板）。
    pub async fn meta(&self, id: i64) -> Result<Vec<AssetParam>, AssetError> {
        // 先证存在性（空清单与不存在必须可区分——404 与 [] 是两回事）。
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM reports WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(_) = exists else {
            return Err(AssetError::NotFound(id));
        };
        let rows = sqlx::query(
            "SELECT name, param_type, data_type, default_val, description FROM report_params WHERE report_id = ?1 ORDER BY rowid",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let param_type = r.get::<i64, _>("param_type") as i8;
                AssetParam {
                    name: r.get::<String, _>("name"),
                    is_static: param_type == 0,
                    param_type,
                    data_type: r.get::<i64, _>("data_type") as i8,
                    default_val: r.get::<Option<String>, _>("default_val"),
                    description: r.get::<Option<String>, _>("description"),
                }
            })
            .collect())
    }

    /// 取原始 BLOB（为后续「从资产库渲染」铺路——本期只有单测消费）。
    pub async fn blob(&self, id: i64) -> Result<Vec<u8>, AssetError> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT rpx FROM reports WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|(b,)| b).ok_or(AssetError::NotFound(id))
    }
}

/// `Param` → 清单条目的纯映射（单测直调，免起库）。
pub(crate) fn param_entry(p: &rpx_core::param::Param) -> Option<AssetParam> {
    let name = p.name.clone()?;
    Some(AssetParam {
        is_static: p.param_type == 0,
        name,
        param_type: p.param_type,
        data_type: p.data_type,
        default_val: p.value.clone(),
        description: p.description.clone(),
    })
}

/// `ReportDefine` 的参数清单 → 条目（`put` 内联逻辑的纯函数化）。
pub(crate) fn param_entries(rd: &ReportDefine) -> Vec<AssetParam> {
    rd.params.iter().filter_map(param_entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 夹具 `full.rpx`：`Fixtures.java:261-266` 声明了两个**静态**参数
    /// （`startDate` DT_STRING 默认 "2024-01-01"、`topN` DT_INT 默认 "10"）。
    fn full_rpx() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/full.rpx"
        ))
        .unwrap()
    }

    #[tokio::test]
    async fn put_rejects_bad_magic_loudly() {
        let store = AssetStore::open("sqlite::memory:").await.unwrap();
        let err = store
            .put("bad", b"NOT_AN_RPX_FILE_AT_ALL")
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::Parse(_)), "{err}");
        // 拒收即不落库。
        assert!(store.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn put_parses_param_manifest_and_meta_round_trips() {
        let store = AssetStore::open("sqlite::memory:").await.unwrap();
        let id = store.put("full", &full_rpx()).await.unwrap();
        let meta = store.meta(id).await.unwrap();
        let names: Vec<&str> = meta.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["startDate", "topN"]);
        for p in &meta {
            assert!(p.is_static, "{} 应为静态参数（PARAM_NORMAL=0）", p.name);
            assert!(p.param_type == 0);
        }
        assert_eq!(meta[0].default_val.as_deref(), Some("2024-01-01"));
        // `Types2.DT_STRING = 11` / `DT_INT = 1`（Types2.java:22/:32），原样透传不归并。
        assert_eq!(meta[0].data_type, 11);
        assert_eq!(meta[1].data_type, 1);
        assert_eq!(meta[1].default_val.as_deref(), Some("10"));
        // blob 原样往返（BLOB 是唯一事实源）。
        assert_eq!(store.blob(id).await.unwrap(), full_rpx());
    }

    #[tokio::test]
    async fn list_and_missing_id_behave() {
        let store = AssetStore::open("sqlite::memory:").await.unwrap();
        let id = store.put("full", &full_rpx()).await.unwrap();
        let items = store.list().await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "full");
        assert_eq!(items[0].id, id);
        assert_eq!(items[0].version, 1);
        // 不存在：meta 与 blob 都显式 404 语义（与空清单可区分）。
        assert!(matches!(
            store.meta(999).await,
            Err(AssetError::NotFound(999))
        ));
        assert!(matches!(
            store.blob(999).await,
            Err(AssetError::NotFound(999))
        ));
    }

    /// 参数分组的纯映射：静态/动态/属性三类原样保存，`is_static` 只认 0。
    #[test]
    fn param_entry_maps_all_three_kinds() {
        use rpx_core::param::Param;
        let mk = |t: i8, name: &str| Param {
            param_type: t,
            data_type: 1,
            name: Some(name.into()),
            description: None,
            value: None,
        };
        assert!(param_entry(&mk(0, "a")).unwrap().is_static);
        assert!(!param_entry(&mk(1, "b")).unwrap().is_static);
        assert!(!param_entry(&mk(2, "c")).unwrap().is_static);
        // 无名参数（name 槽 null）跳过——清单键必须是名字。
        let mut anon = mk(0, "");
        anon.name = None;
        assert!(param_entry(&anon).is_none());
    }
}
