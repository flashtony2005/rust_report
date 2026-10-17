mod args;
mod commands;
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
    let (code, value) = envelope(
        name,
        commands::execute(cli.command).await,
        started.elapsed().as_millis(),
    );
    if cli.output == "text" {
        commands::print_text(&value);
    } else {
        emit(&value);
    }
    std::process::exit(code);
}
