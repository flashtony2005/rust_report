//! HTML → PDF：借用本机 Chrome / Chromium / Edge 的打印引擎。
//!
//! 为什么不自己写 PDF：分页、CJK 字体子集嵌入、字距，任何一项自研都是月级工程，
//! 而本机浏览器已经把"HTML 按 `@page` 分页 + 中文字体嵌入"做完了。这与
//! `docs/platform-blueprint-fde-opc.md` 的 ADR「PDF 复用同一份打印 HTML，不另写
//! 分页引擎」一致：**HTML 是唯一渲染真相，PDF 只是它的一个落盘格式**。
//!
//! 代价说清楚：① 依赖外部浏览器——找不到就**明确失败**，绝不静默降级成"没 PDF
//! 但成功"；② 子进程是同步阻塞的（CLI 场景无所谓；服务面要用需套 `spawn_blocking`）。

use std::path::{Path, PathBuf};
use std::process::Command;

/// 显式指定浏览器路径（装了非标准位置时用）。
pub const ENV: &str = "RPX_CHROME";

const APP_PATHS: [&str; 4] = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
];

const BIN_NAMES: [&str; 6] = [
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "microsoft-edge",
    "msedge",
];

/// 本机可用的浏览器；`None` 表示 PDF 导出不可用。
pub fn chrome() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os(ENV) {
        let path = PathBuf::from(explicit);
        return path.is_file().then_some(path);
    }
    for candidate in APP_PATHS {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        for name in BIN_NAMES {
            let path = dir.join(name);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

/// 渲染 HTML 为 PDF 字节。失败返回**可直接展示给用户**的原因（进 `EXPORT_FAILED`）。
pub fn render(html: &str) -> Result<Vec<u8>, String> {
    let binary = chrome().ok_or_else(|| {
        format!(
            "未找到 Chrome/Chromium/Edge，无法导出 PDF；可设环境变量 {ENV}=<浏览器可执行文件路径>"
        )
    })?;
    let dir = std::env::temp_dir().join(format!("rpx-pdf-{}", crate::files::unique()));
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("无法创建临时目录 {}: {e}", dir.display()))?;
    let page = dir.join("page.html");
    let pdf = dir.join("out.pdf");
    std::fs::write(&page, html).map_err(|e| format!("无法写入临时 HTML: {e}"))?;
    let output = Command::new(&binary)
        .arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--no-sandbox")
        .arg("--no-first-run")
        .arg("--no-pdf-header-footer")
        .arg("--virtual-time-budget=5000")
        .arg(format!("--print-to-pdf={}", pdf.display()))
        .arg(file_url(&page))
        .output()
        .map_err(|e| format!("无法启动 {}: {e}", binary.display()))?;
    let bytes = std::fs::read(&pdf).ok();
    let _ = std::fs::remove_dir_all(&dir);
    match bytes {
        Some(bytes) if bytes.starts_with(b"%PDF") => Ok(bytes),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail: String = stderr
                .chars()
                .rev()
                .take(400)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            Err(format!(
                "浏览器没有产出 PDF（exit={:?}）：{}",
                output.status.code(),
                tail.trim()
            ))
        }
    }
}

/// 本地文件 → `file://` URL。Chromium 不认裸路径里的空格与中文，必须百分号编码。
fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy();
    let mut out = String::from("file://");
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::file_url;
    use std::path::Path;

    #[test]
    fn file_url_percent_encodes_spaces_and_cjk() {
        assert_eq!(
            file_url(Path::new("/tmp/a b.html")),
            "file:///tmp/a%20b.html"
        );
        let url = file_url(Path::new("/tmp/报表.html"));
        assert!(url.starts_with("file:///tmp/"));
        assert!(url.ends_with(".html"));
        assert!(!url.contains('报'));
    }
}
