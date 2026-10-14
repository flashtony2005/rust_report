//! rpx-server —— ReportLite 报表服务端（M3）。
//!
//! 两个子命令（库面见 `lib.rs`）：
//!
//! * `snapshot`：模板 + 数据集 → 快照 JSON 文件/stdout（M4 开发期数据管道）；
//! * `serve`：起 HTTP 服务，`POST /render` 主链路（M3-05，`docs/spec-m3.md` §3.2）。
//!
//! 用法：
//!
//! ```text
//! rpx-server snapshot <file.rpx> [-o out.json]
//! rpx-server serve [--addr 127.0.0.1:8090] [--db rpx.db]
//! ```
//!
//! 输出 Univer `IWorkbookData` JSON —— 契约 `docs/spec-m4.md` §4（字段表）。

use std::process::ExitCode;

use rpx_server::{http, snapshot};

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match run(&args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// 手写解析（与 `rpx-cli` 同风格，不引第三方参数库）。
async fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd: Option<&str> = None;
    let mut path: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut addr: Option<&str> = None;
    let mut db: Option<&str> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            cmd_name @ ("snapshot" | "serve") => cmd = Some(cmd_name),
            "-o" => {
                i += 1;
                out = args.get(i).map(String::as_str);
            }
            "--addr" => {
                i += 1;
                addr = args.get(i).map(String::as_str);
            }
            "--db" => {
                i += 1;
                db = args.get(i).map(String::as_str);
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
        Some("serve") => serve_cmd(addr, db).await,
        _ => {
            eprintln!(
                "usage: rpx-server snapshot <file.rpx> [-o out.json]\n       \
                 rpx-server serve [--addr 127.0.0.1:8090] [--db rpx.db]"
            );
            std::process::exit(2);
        }
    }
}

/// `snapshot` 子命令：模板 + 数据集 → 计算 → 快照 JSON。
///
/// 步骤与 `/render` 主链路（`spec-m3.md` §3.2 的 ①②③④⑤..⑩）同构，
/// 只是 ① 从「multipart 上传」换成「本地文件」。
fn snapshot_cmd(path: Option<&str>, out: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = path else {
        eprintln!("usage: rpx-server snapshot <file.rpx> [-o out.json]");
        std::process::exit(2);
    };
    let file = std::fs::File::open(path)?;
    let rd = rpx_core::open(file)?;
    // 数据集：模板 `FileDataSetConfig` 指向的 CSV（与 `.rpx` 同目录）。
    // 纯字面量模板无文件数据集 ⇒ 空切片，`calculate` 照常跑（M2 同口径）。
    let dir = std::path::Path::new(path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let ds = rpx_engine::dataset::load_datasets(dir, &rd)?;
    let mut grid = rpx_engine::calculate(&rd, &ds)?;
    // M3-06：与 `/render` 同构——pagination 覆盖为**模板真值口径**（纸码表 + 边距）。
    // `calculate()` 内的是探针口径（夹具对拍基准），dev bin 不是对拍入口 ⇒ 覆盖。
    let pg = rpx_engine::pager::paginate_with_setup(&grid, &rd.print_setup)?;
    grid.page_count = pg.page_count();
    grid.pagination = Some(pg);
    let name = std::path::Path::new(path)
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

/// `serve` 子命令：HTTP 服务（M3-05 主链路 + M3-04 资产库三路由）。
/// 默认 `127.0.0.1:8090`（端口约定 `docs/spec-m4.md` §3：web dev 5288 /
/// server 8090）；资产库默认 `rpx.db`（蓝图 §7.1 的单文件部署）。
async fn serve_cmd(addr: Option<&str>, db: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let addr: std::net::SocketAddr = addr.unwrap_or("127.0.0.1:8090").parse()?;
    let db = db.unwrap_or("rpx.db");
    let store = std::sync::Arc::new(rpx_ds::AssetStore::open(&format!("sqlite://{db}")).await?);
    eprintln!(
        "rpx-server listening on http://{addr}（POST /render + /reports 资产三路由；\
         资产库 {db}，契约 docs/spec-m3.md §3.2）"
    );
    http::serve(addr, store).await?;
    Ok(())
}
