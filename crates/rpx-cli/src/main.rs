mod args;
mod commands;
mod doctor;
use clap::Parser;
use rpx_runtime::error::{envelope, Error};
use std::io::Write;

/// 写一行 JSON；stdout 被提前关闭（EPIPE）时不 panic。
fn emit(value: &serde_json::Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
}
#[tokio::main]
async fn main() {
    let started = std::time::Instant::now();
    let cli = match args::Cli::try_parse() {
        Ok(v) => v,
        Err(e) => {
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                let mut out = std::io::stdout().lock();
                let _ = write!(out, "{e}");
                return;
            }
            let (_, value) = envelope(
                "arguments",
                Err(Error::new(2, "USAGE", "arguments", e)),
                started.elapsed().as_millis(),
            );
            emit(&value);
            std::process::exit(2);
        }
    };
    let name = cli.command.name();
    let (mut code, value) = envelope(
        name,
        commands::execute(cli.command).await,
        started.elapsed().as_millis(),
    );
    // `lint --strict` 这类命令：本身跑成功了（ok=true），但"有告警"要让 CI 能判失败，
    // 所以由命令自己在 data.gate.exit_code 里给出退出码，这里只负责采纳。
    if code == 0 {
        if let Some(exit) = value["data"]["gate"]["exit_code"].as_i64() {
            code = exit as i32;
        }
    }
    if cli.output == "text" {
        commands::print_text(&value);
    } else {
        emit(&value);
    }
    std::process::exit(code);
}
