//! 数据源注册表（M3-05 的 SQL 数据集服务面延伸）：
//! `dsn → SqlSource` 的进程内映射，`serve` 启动时从 JSON 配置建池。
//!
//! # 产品同构
//!
//! 产品把连接三要素放 `raqsoftConfig.xml` 的 `<DBList>`（配置文件，**不进
//! .rpx**，`SQLDataSetConfig` 只存 `data_source_name` 引用）；引擎经
//! `ctx.getConnectionFactory(dsn)` 取连接。本注册表即 rpx 版 DBList：
//! 连接由**调用方（serve 编排层）**注册，引擎/快照层只见 dsn（G8 分层不变）。
//!
//! # 配置格式（`--datasources <path.json>`）
//!
//! ```json
//! {
//!   "kingbase": "postgres://user:pass@192.168.1.77:54323/test",
//!   "local":    { "url": "postgres://192.168.1.77:54323/test",
//!                 "user": "system", "pass": "..." }
//! }
//! ```
//!
//! 字符串形态 = 完整连接串（凭据内嵌）；对象形态 = 三要素分写（注册表负责
//! 把 userinfo 拼进 URL——`scheme://user:pass@rest`，凭据含 `@`/`:` 需自行
//! URL 编码，与主流 JDBC URL 同约定）。空对象 = 不注册任何数据源（合法：
//! 模板全用文件数据集的服务无需配置）。
//!
//! 建池失败**快速失败**（启动期报错退出，不带病服务）——与 AssetStore
//! 的 sqlite 路径口径一致：配置错误在启动期暴露，不在首个请求期爆。

use std::collections::HashMap;

use rpx_ds::SqlSource;

/// 注册表：`dsn → 池`。不可变（启动期建好后只读），`Arc` 跨 handler 共享。
/// `Debug` 手写（连接池不进输出——凭据不入日志，与「凭据不入模板」同纪律）。
pub struct DataSourceRegistry {
    map: HashMap<String, SqlSource>,
}

impl std::fmt::Debug for DataSourceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceRegistry")
            .field("dsns", &self.names())
            .finish()
    }
}

/// 配置错误（JSON 形态坏 / URL scheme 不在方言表）。
#[derive(Debug)]
pub struct DsConfigError(pub String);

impl std::fmt::Display for DsConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DsConfigError {}

impl DataSourceRegistry {
    /// 从 JSON 配置文本建池（逐条连接，失败即 Err）。
    pub async fn from_json(text: &str) -> Result<Self, DsConfigError> {
        let j: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| DsConfigError(format!("数据源配置不是合法 JSON：{e}")))?;
        let serde_json::Value::Object(map) = j else {
            return Err(DsConfigError(
                "数据源配置必须是 JSON 对象（键 = 数据源名）".to_string(),
            ));
        };
        let mut out = HashMap::new();
        for (name, spec) in map {
            let url = match &spec {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Object(m) => {
                    let url = m
                        .get("url")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| DsConfigError(format!("数据源 {name} 缺 url 字段")))?;
                    let user = m.get("user").and_then(|v| v.as_str());
                    let pass = m.get("pass").and_then(|v| v.as_str());
                    compose_url(url, user, pass)
                }
                _ => {
                    return Err(DsConfigError(format!(
                        "数据源 {name} 的值必须是 URL 字符串或 {{url,user,pass}} 对象"
                    )))
                }
            };
            let src = SqlSource::connect(&url)
                .await
                .map_err(|e| DsConfigError(format!("数据源 {name} 连接失败：{e}")))?;
            out.insert(name, src);
        }
        Ok(DataSourceRegistry { map: out })
    }

    /// 从配置文件建池（`serve --datasources <path>`）。
    pub async fn from_file(path: &std::path::Path) -> Result<Self, DsConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| DsConfigError(format!("读数据源配置 {} 失败：{e}", path.display())))?;
        Self::from_json(&text).await
    }

    /// 取数据源（`load_datasets_full` 的 SQL 委托预取阶段用）。
    pub fn get(&self, dsn: &str) -> Option<&SqlSource> {
        self.map.get(dsn)
    }

    /// 已登记的数据源名（错误消息里给用户看有哪些可选）。
    pub fn names(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.map.keys().map(String::as_str).collect();
        v.sort_unstable();
        v
    }
}

/// 对象形态拼 URL：`scheme://[user:pass@]rest`。
fn compose_url(url: &str, user: Option<&str>, pass: Option<&str>) -> String {
    match (user, pass) {
        (Some(u), Some(p)) => match url.split_once("://") {
            Some((scheme, rest)) => format!("{scheme}://{u}:{p}@{rest}"),
            None => url.to_string(),
        },
        _ => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_url_inserts_userinfo() {
        assert_eq!(
            compose_url("postgres://h:5432/db", Some("system"), Some("pw")),
            "postgres://system:pw@h:5432/db"
        );
        // 缺任一要素 = 原样（不拼半个 userinfo）。
        assert_eq!(
            compose_url("postgres://h:5432/db", Some("u"), None),
            "postgres://h:5432/db"
        );
    }

    #[test]
    fn empty_object_is_a_valid_empty_registry() {
        // 文件数据集模板的服务无需配置任何数据源——空对象合法。
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let reg = rt.block_on(DataSourceRegistry::from_json("{}")).unwrap();
        assert_eq!(reg.names(), Vec::<&str>::new());
    }

    #[tokio::test]
    async fn bad_shape_and_bad_url_fail_loudly() {
        let err = DataSourceRegistry::from_json("[1]").await.unwrap_err();
        assert!(err.0.contains("JSON 对象"), "{err}");
        // 字符串形态但 scheme 不在方言表：连接期显式报错（带数据源名）。
        let err = DataSourceRegistry::from_json(r#"{"kb": "oracle://x"}"#)
            .await
            .unwrap_err();
        assert!(err.0.contains("kb"), "{err}");
        // 对象形态缺 url：点名数据源名。
        let err = DataSourceRegistry::from_json(r#"{"kb": {"user": "u"}}"#)
            .await
            .unwrap_err();
        assert!(err.0.contains("缺 url"), "{err}");
    }
}
