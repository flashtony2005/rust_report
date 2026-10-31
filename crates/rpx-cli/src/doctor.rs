//! `doctor` 的检查项。独立成模块有两个原因：`commands.rs` 触到 500 行闸门；
//! 更重要的是这里的东西都是"自检"，与命令分发无关，混在一起会让噪声变大。
use rpx_runtime::{error::Error, files};
use serde_json::{json, Value as J};
use std::path::Path;

fn library(path: Option<&Path>) -> J {
    let Some(path) = path else {
        return json!({"name":"library","ok":true,"detail":"not requested"});
    };
    let checked = (|| {
        let root = files::absolute(path)?;
        let meta = files::read_json(&root.join(".library.json"))?;
        if meta["version"] != 1 {
            return Err(Error::new(
                3,
                "LIBRARY_INVALID",
                "doctor",
                "不支持的报表库版本",
            ));
        }
        let probe = root.join(format!(".doctor-{}", files::unique()));
        files::create(&probe, b"ok")?;
        let _ = std::fs::remove_file(&probe);
        Ok::<_, Error>(root)
    })();
    match checked {
        Ok(root) => json!({"name":"library","ok":true,"detail":root,"writable":true}),
        Err(e) => json!({"name":"library","ok":false,"detail":e.diagnostic}),
    }
}

fn datasources(path: Option<&Path>) -> J {
    let Some(path) = path else {
        return json!({"name":"datasources","ok":true,"detail":"not requested","connected":false});
    };
    let checked = (|| {
        let value: J = serde_json::from_slice(&std::fs::read(path)?)?;
        let Some(map) = value.as_object() else {
            return Err(Error::new(
                3,
                "DATASOURCES_INVALID",
                "doctor",
                "数据源配置必须是 JSON 对象",
            ));
        };
        for (name, spec) in map {
            let url = match spec {
                J::String(url) => url.as_str(),
                J::Object(m) => m.get("url").and_then(J::as_str).ok_or_else(|| {
                    Error::new(3, "DATASOURCES_INVALID", "doctor", format!("{name} 缺 url"))
                })?,
                _ => {
                    return Err(Error::new(
                        3,
                        "DATASOURCES_INVALID",
                        "doctor",
                        format!("{name} 必须是 URL 或 {{url,user,pass}}"),
                    ))
                }
            };
            if !url.contains("://") {
                return Err(Error::new(
                    3,
                    "DATASOURCES_INVALID",
                    "doctor",
                    format!("{name} 的 URL 缺 scheme"),
                ));
            }
        }
        Ok::<_, Error>(map.len())
    })();
    match checked {
        Ok(count) => {
            json!({"name":"datasources","ok":true,"detail":format!("{count} datasource(s)"),"connected":false})
        }
        Err(e) => json!({"name":"datasources","ok":false,"detail":e.diagnostic,"connected":false}),
    }
}

/// 在**源码仓库里**运行时，读出 `crates/rpx-cli/Cargo.toml` 的版本。
pub(crate) fn source_version() -> Option<(String, std::path::PathBuf)> {
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..6 {
        let manifest = dir.join("crates/rpx-cli/Cargo.toml");
        if manifest.is_file() {
            let text = std::fs::read_to_string(&manifest).ok()?;
            let mut in_package = false;
            for line in text.lines() {
                let line = line.trim();
                if line.starts_with('[') {
                    in_package = line == "[package]";
                    continue;
                }
                if in_package {
                    if let Some(rest) = line.strip_prefix("version") {
                        return rest
                            .split('"')
                            .nth(1)
                            .map(|v| (v.to_string(), manifest.clone()));
                    }
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// 二进制版本 vs 源码版本。文档跟着源码走、二进制可能停在几天前，
/// 而 Agent 最信任文档——所以"落后"必须能被自己发现。
fn binary() -> J {
    let version = env!("CARGO_PKG_VERSION");
    match source_version() {
        Some((source, path)) => {
            let stale = source != version;
            json!({
                "name": "binary",
                "ok": !stale,
                "version": version,
                "source_version": source,
                "source_manifest": path.display().to_string(),
                "stale": stale,
                "detail": if stale {
                    format!("二进制 {version} 落后于源码 {source}；先 make install-local，否则按文档写的参数可能不存在")
                } else {
                    "与源码一致".to_string()
                },
            })
        }
        None => json!({
            "name": "binary",
            "ok": true,
            "version": version,
            "source_version": J::Null,
            "stale": false,
            "detail": "当前目录不在 rpx 源码仓库内，无法比对源码版本",
        }),
    }
}

/// PDF 导出依赖外部浏览器。这里报的是"**这台机器**能不能出 PDF"：
/// `ok` 表示检查本身跑通了，能力结论在 `available`——与 binary 检查同一口径
/// （"无法比对"不等于"不一致"）。
fn pdf() -> J {
    match rpx_runtime::pdf::chrome() {
        Some(path) => json!({
            "name": "pdf",
            "ok": true,
            "available": true,
            "path": path.display().to_string(),
            "detail": "--export pdf 可用",
        }),
        None => json!({
            "name": "pdf",
            "ok": true,
            "available": false,
            "path": J::Null,
            "detail": format!(
                "未找到 Chrome/Chromium/Edge；--export pdf 不可用，可设 {} 指定浏览器路径",
                rpx_runtime::pdf::ENV
            ),
        }),
    }
}

pub fn checks(library_path: Option<&Path>, datasources_path: Option<&Path>) -> Vec<J> {
    vec![
        binary(),
        pdf(),
        library(library_path),
        datasources(datasources_path),
    ]
}
