use super::*;
use crate::catalog;

pub fn init(root: &Path, presets: bool) -> Result<J> {
    let root = files::absolute(root)?;
    let parent = root.parent().unwrap();
    fs::create_dir_all(parent)?;
    let guard = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(parent.join(".rpx-init.lock"))?;
    guard.lock()?;
    if root.exists() {
        return Err(Error::new(
            6,
            "DESTINATION_EXISTS",
            "library",
            "目标库已存在",
        ));
    }
    let staged = parent.join(format!(".rpx-{}", files::unique()));
    fs::create_dir(&staged)?;
    let result = (|| {
        files::create(&staged.join(".library.json"), b"{\"version\":1}\n")?;
        files::create(&staged.join(".gitignore"), b".runs/\n.lock\n.*.pending\n")?;
        if presets {
            for example in catalog::examples() {
                catalog::write_package(example.id, &staged.join(example.id), true)?;
            }
        }
        fs::rename(&staged, &root)?;
        Ok(json!({"library":root,"presets":presets}))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(staged);
    }
    result
}
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let ft = e.file_type()?;
        let dest = to.join(e.file_name());
        if ft.is_symlink() {
            return Err(Error::new(
                6,
                "PACKAGE_SYMLINK",
                "library",
                "报表包内不允许符号链接",
            ));
        }
        if ft.is_dir() {
            copy_tree(&e.path(), &dest)?;
        } else {
            fs::copy(e.path(), dest)?;
        }
    }
    Ok(())
}
pub fn copy(root: &Path, name: &str, new: &str) -> Result<J> {
    id(new)?;
    let _guard = lock(root)?;
    let original = capture(root, name)?;
    if root.join(new).exists() {
        return Err(Error::new(6, "ID_EXISTS", "library", new));
    }
    let staged = root.join(format!(".{}.pending", files::unique()));
    let result = (|| {
        copy_tree(&root.join(name), &staged)?;
        let mut manifest = original.manifest;
        manifest["id"] = json!(new);
        files::atomic_json(&staged.join("manifest.json"), &manifest)?;
        fs::rename(&staged, root.join(new))?;
        show_captured(root, new, &capture(root, new)?)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(staged);
    }
    result
}
pub fn update(
    root: &Path,
    name: &str,
    expected: &str,
    title: Option<&str>,
    description: Option<&str>,
) -> Result<J> {
    let _guard = lock(root)?;
    let mut c = capture(root, name)?;
    matches(&c.source.revision, expected)?;
    if let Some(v) = title {
        c.manifest["name"] = json!(v);
    }
    if let Some(v) = description {
        c.manifest["description"] = json!(v);
    }
    files::atomic_json(&root.join(name).join("manifest.json"), &c.manifest)?;
    show_captured(root, name, &capture(root, name)?)
}
fn dependencies(rd: &rpx_core::ReportDefine, data: Option<&Path>) -> Vec<J> {
    let mut dependencies = Vec::new();
    for d in &rd.datasets {
        if let Some(f) = &d.file {
            let n = f.file_name.as_deref().unwrap_or("");
            let found = data.is_some_and(|p| p.join(n).is_file());
            dependencies.push(json!({"file":n,"available":found}));
        }
    }
    dependencies
}

fn copy_dependencies(staged: &Path, data: &Path, dependencies: &[J]) -> Result<()> {
    for d in dependencies {
        if d["available"] == true {
            let n = d["file"].as_str().unwrap();
            let dest = files::relative(&staged.join("data"), n)?;
            fs::create_dir_all(dest.parent().unwrap())?;
            fs::copy(data.join(n), dest)?;
        }
    }
    Ok(())
}

/// 从普通 JSON 定义创建库内报表（JSON-native 入口）。
pub fn import_json(
    root: &Path,
    file: &Path,
    name: &str,
    data: Option<&Path>,
    dry: bool,
) -> Result<J> {
    id(name)?;
    let template = crate::format::normalize(&files::read_json(file)?)?;
    let (rd, _) = definition::parse(&template)?;
    let dependencies = dependencies(&rd, data);
    if dry {
        return Ok(json!({"dry_run":true,"template":template,"dependencies":dependencies}));
    }
    let _guard = lock(root)?;
    if root.join(name).exists() {
        return Err(Error::new(6, "ID_EXISTS", "library", name));
    }
    let staged = root.join(format!(".{}.pending", files::unique()));
    fs::create_dir(&staged)?;
    let result = (|| {
        fs::create_dir(staged.join("data"))?;
        files::create(
            &staged.join("template.json"),
            &files::json_bytes(&template)?,
        )?;
        files::create(
            &staged.join("manifest.json"),
            &files::json_bytes(
                &json!({"version":1,"id":name,"name":name,"description":"JSON import","tags":[],"template":"template.json","data_dir":"data"}),
            )?,
        )?;
        if let Some(data) = data {
            copy_dependencies(&staged, data, &dependencies)?;
        }
        fs::rename(&staged, root.join(name))?;
        let mut value = show_captured(root, name, &capture(root, name)?)?;
        value["dependencies"] = json!(dependencies);
        Ok(value)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(staged);
    }
    result
}

/// 删除整份报表包；要求显式 revision，避免 agent 误删。
pub fn delete(root: &Path, name: &str, expected: &str) -> Result<J> {
    let _guard = lock(root)?;
    let c = capture(root, name)?;
    matches(&c.source.revision, expected)?;
    let staged = root.join(format!(".{}.deleting-{}", name, files::unique()));
    fs::rename(root.join(name), &staged)?;
    if let Err(e) = fs::remove_dir_all(&staged) {
        return Err(Error::new(6, "IO_ERROR", "library", e));
    }
    Ok(json!({"deleted":name}))
}

/// 保留最近 `keep` 次运行，删除更早的 `.runs/<id>/<run-id>`。
pub fn prune(root: &Path, name: &str, keep: usize) -> Result<J> {
    id(name)?;
    if keep == 0 {
        return Err(Error::new(2, "USAGE", "arguments", "keep 必须 >= 1"));
    }
    let _guard = lock(root)?;
    capture(root, name)?;
    let dir = root.join(".runs").join(name);
    if !dir.exists() {
        return Ok(json!({"pruned":0,"kept":0}));
    }
    let mut entries: Vec<(String, String)> = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let run_id = entry.file_name().to_string_lossy().into_owned();
        let record = files::read_json(&entry.path().join("run.json"))?;
        entries.push((
            run_id,
            record["started_at"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        ));
    }
    entries.sort_by_key(|a| std::cmp::Reverse(timestamp_key(&a.1)));
    let mut pruned = 0;
    for (run_id, _) in entries.iter().skip(keep) {
        fs::remove_dir_all(dir.join(run_id))?;
        pruned += 1;
    }
    Ok(json!({"pruned":pruned,"kept":entries.len().saturating_sub(pruned)}))
}

#[cfg(feature = "legacy-rpx")]
pub fn import(root: &Path, file: &Path, name: &str, data: Option<&Path>, dry: bool) -> Result<J> {
    id(name)?;
    // 迁移入口走 Preserve 快照：v1 未建模字段由 extensions.rpx.legacy 承载，
    // 因而不再用 import::convert 的保守映射阻塞产品模板。
    let converted = crate::format::rpx_to_json(
        &fs::read(file)?,
        crate::format::Options {
            mode: crate::format::Mode::PreserveLegacy,
        },
    )?;
    let template = converted.document;
    let warnings = converted.warnings;
    let (rd, _) = definition::parse(&template)?;
    let dependencies = dependencies(&rd, data);
    if dry {
        return Ok(json!({
            "dry_run":true,
            "template":template,
            "dependencies":dependencies,
            "warnings":warnings,
        }));
    }
    let _guard = lock(root)?;
    if root.join(name).exists() {
        return Err(Error::new(6, "ID_EXISTS", "library", name));
    }
    let staged = root.join(format!(".{}.pending", files::unique()));
    fs::create_dir(&staged)?;
    let result = (|| {
        fs::create_dir(staged.join("data"))?;
        files::create(
            &staged.join("template.json"),
            &files::json_bytes(&template)?,
        )?;
        files::create(
            &staged.join("manifest.json"),
            &files::json_bytes(
                &json!({"version":1,"id":name,"name":name,"description":"RPX import","tags":[],"template":"template.json","data_dir":"data"}),
            )?,
        )?;
        if let Some(data) = data {
            copy_dependencies(&staged, data, &dependencies)?;
        }
        fs::rename(&staged, root.join(name))?;
        let mut value = show_captured(root, name, &capture(root, name)?)?;
        value["dependencies"] = json!(dependencies);
        value["warnings"] = json!(warnings);
        Ok(value)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(staged);
    }
    result
}
