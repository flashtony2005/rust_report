//! Directory library. CLI and HTTP use this exact lock/revision protocol.
use crate::{
    definition,
    error::{Error, Result},
    files, run,
};
use serde_json::{json, Value as J};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};
mod manage;
#[cfg(feature = "legacy-rpx")]
pub use manage::import;
pub use manage::{copy, init, update};

pub fn id(value: &str) -> Result<()> {
    if value.is_empty()
        || !value.as_bytes()[0].is_ascii_lowercase() && !value.as_bytes()[0].is_ascii_digit()
        || !value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
    {
        return Err(Error::new(
            2,
            "ID_INVALID",
            "library",
            "ID 必须匹配 [a-z0-9][a-z0-9_-]*",
        ));
    }
    Ok(())
}
pub(super) fn lock(root: &Path) -> Result<File> {
    if files::read_json(&root.join(".library.json"))?["version"] != 1 {
        return Err(Error::new(
            3,
            "LIBRARY_INVALID",
            "library",
            "不支持的报表库版本",
        ));
    }
    let f = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join(".lock"))?;
    f.lock()?;
    Ok(f)
}
pub(super) struct Captured {
    pub manifest: J,
    pub source: run::Source,
    pub template_path: PathBuf,
}
pub(super) fn capture(root: &Path, name: &str) -> Result<Captured> {
    id(name)?;
    let base = root.join(name);
    let manifest_bytes = fs::read(base.join("manifest.json"))?;
    let manifest: J = serde_json::from_slice(&manifest_bytes)?;
    let allowed = [
        "version",
        "id",
        "name",
        "description",
        "tags",
        "template",
        "data_dir",
        "params_file",
    ];
    if manifest["version"] != 1
        || manifest["id"].as_str() != Some(name)
        || manifest
            .as_object()
            .is_none_or(|m| m.keys().any(|k| !allowed.contains(&k.as_str())))
    {
        return Err(Error::new(
            3,
            "MANIFEST_INVALID",
            "library",
            "清单版本、ID 或字段不合法",
        ));
    }
    let string = |key: &str| {
        manifest[key]
            .as_str()
            .ok_or_else(|| Error::new(3, "MANIFEST_INVALID", "library", format!("清单缺少 {key}")))
    };
    let template_name = string("template")?;
    if template_name == "manifest.json"
        || manifest["description"].as_str().is_none()
        || manifest["tags"]
            .as_array()
            .is_none_or(|v| v.iter().any(|t| !t.is_string()))
    {
        return Err(Error::new(
            3,
            "MANIFEST_INVALID",
            "library",
            "清单路径、description 或 tags 不合法",
        ));
    }
    let template_path = files::relative(&base, template_name)?;
    let bytes = fs::read(&template_path)?;
    let template = serde_json::from_slice(&bytes)?;
    let mut sources: Vec<(String, Vec<u8>)> = vec![
        ("manifest.json".into(), manifest_bytes),
        (template_name.into(), bytes),
    ];
    let params = if let Some(p) = manifest.get("params_file") {
        let name = p.as_str().ok_or_else(|| {
            Error::new(3, "MANIFEST_INVALID", "library", "params_file 必须是字符串")
        })?;
        if name == "manifest.json" || name == template_name {
            return Err(Error::new(
                3,
                "MANIFEST_INVALID",
                "library",
                "默认参数必须使用独立文件",
            ));
        }
        let bytes = fs::read(files::relative(&base, name)?)?;
        let value = serde_json::from_slice(&bytes)?;
        crate::parameters::parse(&value)?;
        sources.push((name.into(), bytes));
        value
    } else {
        json!({})
    };
    sources.sort_by(|a, b| a.0.cmp(&b.0));
    let mut h = Sha256::new();
    for (p, b) in &sources {
        h.update((p.len() as u64).to_be_bytes());
        h.update(p.as_bytes());
        h.update((b.len() as u64).to_be_bytes());
        h.update(b);
    }
    let source = run::Source {
        template,
        name: string("name")?.into(),
        data_dir: if string("data_dir")? == "." {
            base.clone()
        } else {
            files::relative(&base, string("data_dir")?)?
        },
        params,
        report_id: Some(name.into()),
        revision: format!("{:x}", h.finalize()),
        files: sources,
    };
    Ok(Captured {
        manifest,
        source,
        template_path,
    })
}
fn matches(revision: &str, expected: &str) -> Result<()> {
    if revision != expected {
        return Err(Error::new(
            7,
            "REVISION_CONFLICT",
            "revision",
            "源码已变化，请重新读取后处理修改",
        ));
    }
    Ok(())
}
pub fn runs(root: &Path, name: &str) -> Result<Vec<J>> {
    id(name)?;
    let dir = root.join(".runs").join(name);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut records = Vec::new();
    for entry in fs::read_dir(dir)? {
        let p = entry?.path().join("run.json");
        if p.is_file() {
            records.push(files::read_json(&p)?);
        }
    }
    records.sort_by(|a, b| {
        timestamp_key(b["started_at"].as_str().unwrap_or_default())
            .cmp(&timestamp_key(a["started_at"].as_str().unwrap_or_default()))
            .then_with(|| b["run_id"].as_str().cmp(&a["run_id"].as_str()))
    });
    Ok(records)
}
/// RFC3339 时间字符串 → 固定宽度键（小数位补齐），避免 `.99Z` 与 `.9901Z`
/// 的字典序错误。
fn timestamp_key(value: &str) -> String {
    let value = value.trim_end_matches('Z');
    match value.split_once('.') {
        Some((base, fraction)) => format!("{base}.{fraction:0<9}"),
        None => format!("{value}.000000000"),
    }
}

fn state(records: &[J], revision: &str) -> J {
    let successes: Vec<_> = records
        .iter()
        .filter(|r| r["status"] == "succeeded")
        .collect();
    let display = successes
        .iter()
        .find(|r| r["revision"] == revision)
        .copied()
        .or_else(|| successes.first().copied());
    json!({"result_state":match display{None=>"none",Some(r) if r["revision"]==revision=>"current_revision",_=>"older_revision"},
        "display_run_id":display.map(|r|r["run_id"].clone()),"display_run":display,"latest_attempt":records.first()})
}
fn show_captured(root: &Path, name: &str, c: &Captured) -> Result<J> {
    let records = runs(root, name)?;
    let status = state(&records, &c.source.revision);
    Ok(
        json!({"id":name,"manifest":c.manifest,"template":c.source.template,"template_path":files::absolute(&c.template_path)?,"revision":c.source.revision,
        "result_state":status["result_state"],"display_run_id":status["display_run_id"],"display_run":status["display_run"],"latest_attempt":status["latest_attempt"]}),
    )
}
pub fn show(root: &Path, name: &str) -> Result<J> {
    let _guard = lock(root)?;
    let c = capture(root, name)?;
    show_captured(root, name, &c)
}
pub fn list(
    root: &Path,
    query: Option<&str>,
    tag: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<J> {
    let _guard = lock(root)?;
    let mut items = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.path().join("manifest.json").is_file() || name.starts_with('.') {
            continue;
        }
        let c = capture(root, &name)?;
        if query.is_some_and(|q| {
            !c.manifest
                .to_string()
                .to_lowercase()
                .contains(&q.to_lowercase())
        }) {
            continue;
        }
        if tag.is_some_and(|t| {
            !c.manifest["tags"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v == t))
        }) {
            continue;
        }
        let mut value = show_captured(root, &name, &c)?;
        value.as_object_mut().unwrap().remove("template");
        items.push(value);
    }
    items.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    files::page(&items, limit, offset)
}
pub fn save(root: &Path, name: &str, expected: &str, template: &J, dry: bool) -> Result<J> {
    definition::validate(template, None)?;
    let _guard = lock(root)?;
    let c = capture(root, name)?;
    matches(&c.source.revision, expected)?;
    if dry {
        return Ok(json!({"dry_run":true,"revision":expected,"template":template}));
    }
    files::atomic_json(&c.template_path, template)?;
    show_captured(root, name, &capture(root, name)?)
}
pub fn patch(root: &Path, name: &str, expected: &str, patch: &J, dry: bool) -> Result<J> {
    let _guard = lock(root)?;
    let c = capture(root, name)?;
    matches(&c.source.revision, expected)?;
    let template = definition::patch(&c.source.template, patch)?;
    if dry {
        return Ok(json!({"dry_run":true,"revision":expected,"template":template}));
    }
    files::atomic_json(&c.template_path, &template)?;
    show_captured(root, name, &capture(root, name)?)
}
pub fn validate(root: &Path, name: &str) -> Result<J> {
    let _guard = lock(root)?;
    let c = capture(root, name)?;
    definition::validate(&c.source.template, Some(&c.source.params))
}
pub async fn execute(root: &Path, name: &str, expected: &str, options: &run::Options) -> Result<J> {
    let (source, out, record) = {
        let _guard = lock(root)?;
        let mut c = capture(root, name)?;
        matches(&c.source.revision, expected)?;
        if let Some(p) = &options.params {
            c.source.params = p.clone();
        }
        if let Some(p) = &options.data_dir {
            c.source.data_dir = p.clone();
        }
        let parent = root.join(".runs").join(name);
        fs::create_dir_all(&parent)?;
        let out = parent.join(format!("run-{}", files::unique()));
        let record = run::start(&c.source, &out)?;
        (c.source, out, record)
    };
    run::execute(source, options, &out, record).await
}
pub fn run_record(root: &Path, name: &str, run_id: &str) -> Result<J> {
    id(name)?;
    id(run_id)?;
    files::read_json(&root.join(".runs").join(name).join(run_id).join("run.json"))
}
pub fn source_file(root: &Path, name: &str, run_id: &str, path: &str) -> Result<PathBuf> {
    let record = run_record(root, name, run_id)?;
    let relative = format!("source/{path}");
    if !record["source_files"]
        .as_array()
        .is_some_and(|files| files.iter().any(|v| v == &relative))
    {
        return Err(Error::new(
            6,
            "SOURCE_MISSING",
            "result",
            "该源码不在运行记录中",
        ));
    }
    files::relative(&root.join(".runs").join(name).join(run_id), &relative)
}
pub fn artifact(root: &Path, name: &str, run_id: &str, artifact: &str) -> Result<PathBuf> {
    let record = run_record(root, name, run_id)?;
    if record["status"] != "succeeded" {
        return Err(Error::new(
            6,
            "ARTIFACT_UNAVAILABLE",
            "result",
            "运行尚未成功",
        ));
    }
    let path = record["artifacts"][artifact]
        .as_str()
        .ok_or_else(|| Error::new(6, "ARTIFACT_MISSING", "result", "运行未生成该文件"))?;
    files::relative(&root.join(".runs").join(name).join(run_id), path)
}

#[cfg(test)]
mod tests {
    use super::timestamp_key;

    #[test]
    fn timestamp_key_pads_fractional_seconds() {
        assert!(
            timestamp_key("2026-10-17T11:09:07.9901Z") > timestamp_key("2026-10-17T11:09:07.99Z")
        );
        assert_eq!(
            timestamp_key("2026-10-17T11:09:07Z"),
            "2026-10-17T11:09:07.000000000"
        );
    }
}

#[cfg(test)]
mod ordering_tests {
    use super::runs;
    use serde_json::json;

    #[test]
    fn run_history_orders_fractional_timestamps() {
        let root = std::env::temp_dir().join(format!("rpx-order-{}", crate::files::unique()));
        let dir = root.join(".runs").join("x");
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        std::fs::write(
            dir.join("a/run.json"),
            serde_json::to_vec(&json!({"started_at":"2026-10-17T11:09:07.99Z","run_id":"a"}))
                .unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.join("b/run.json"),
            serde_json::to_vec(&json!({"started_at":"2026-10-17T11:09:07.9901Z","run_id":"b"}))
                .unwrap(),
        )
        .unwrap();
        let records = runs(&root, "x").unwrap();
        assert_eq!(records[0]["run_id"], "b");
        assert_eq!(records[1]["run_id"], "a");
        let _ = std::fs::remove_dir_all(root);
    }
}
