use crate::error::{Error, Result};
use serde_json::Value as J;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn unique() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    format!(
        "{:020}-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}
pub fn absolute(path: &Path) -> Result<PathBuf> {
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    })
}
pub fn relative(root: &Path, name: &str) -> Result<PathBuf> {
    let p = Path::new(name);
    if name.is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(Error::new(
            3,
            "PATH_INVALID",
            "library",
            format!("需要包内相对路径：{name}"),
        ));
    }
    Ok(root.join(p))
}
pub fn read_json(path: &Path) -> Result<J> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
pub fn json_bytes(value: &J) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
pub fn create(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}
pub fn atomic_json(path: &Path, value: &J) -> Result<()> {
    let tmp = path.with_extension(format!("{}.pending", unique()));
    create(&tmp, &json_bytes(value)?)?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn page(items: &[J], limit: usize, offset: usize) -> Result<J> {
    if limit == 0 || limit > 500 {
        return Err(Error::new(2, "USAGE", "arguments", "limit 必须在 1..=500"));
    }
    let selected: Vec<_> = items.iter().skip(offset).take(limit).cloned().collect();
    let next = offset.saturating_add(selected.len());
    Ok(
        serde_json::json!({"items":selected,"returned":selected.len(),"total":items.len(),"next_offset":if next<items.len(){Some(next)}else{None}}),
    )
}
