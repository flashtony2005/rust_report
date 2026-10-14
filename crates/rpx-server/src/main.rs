//! rpx-server —— ReportLite 报表服务端（M3）。
//!
//! 当前形态（2026-10-14）：**快照映射 + dev bin**，尚不含 HTTP。
//! HTTP 面（axum + `POST /render`）在 M3-05 接入（`docs/spec-m3.md` §2.1）；
//! 数据源层（`rpx-ds`）在 M3-03；本 bin 的加载路径与将来 server 共用同一代码。
//!
//! 用法：
//!
//! ```text
//! rpx-server snapshot <file.rpx> [-o out.json]
//! ```
//!
//! 输出 Univer `IWorkbookData` JSON —— 契约 `docs/spec-m4.md` §4（字段表）。
//! 前端开发期用它直出真实快照（`spec-m4.md` §6 工作流第 1 步）。

use std::fs::File;
use std::path::Path;
use std::process::ExitCode;

mod snapshot;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// 手写解析（与 `rpx-cli` 同风格，不引第三方参数库）。
fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd: Option<&str> = None;
    let mut path: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "snapshot" => cmd = Some("snapshot"),
            "-o" => {
                i += 1;
                out = args.get(i).map(String::as_str);
            }
            other if !other.starts_with('-') => path = Some(other),
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    match cmd {
        Some("snapshot") => snapshot_cmd(path, out),
        _ => {
            eprintln!("usage: rpx-server snapshot <file.rpx> [-o out.json]");
            std::process::exit(2);
        }
    }
}

/// `snapshot` 子命令：模板 + 数据集 → 计算 → 快照 JSON。
///
/// 步骤与 `/render` 主链路（`spec-m3.md` §3.2 的 ①②③④⑤..⑩）同构，
/// 只是 ① 从「libSQL 取 BLOB」换成「本地文件」。
fn snapshot_cmd(path: Option<&str>, out: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = path else {
        eprintln!("usage: rpx-server snapshot <file.rpx> [-o out.json]");
        std::process::exit(2);
    };
    let file = File::open(path)?;
    let rd = rpx_core::open(file)?;
    // 数据集：模板 `FileDataSetConfig` 指向的 CSV（与 `.rpx` 同目录）。
    // 纯字面量模板无文件数据集 ⇒ 空切片，`calculate` 照常跑（M2 同口径）。
    let dir = Path::new(path).parent().unwrap_or(Path::new("."));
    let ds = rpx_engine::dataset::load_datasets(dir, &rd)?;
    let grid = rpx_engine::calculate(&rd, &ds)?;
    let name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("report");
    let json = snapshot::build(&rd, &grid, name)?;
    match out {
        Some(p) => std::fs::write(p, json)?,
        None => println!("{json}"),
    }
    Ok(())
}
