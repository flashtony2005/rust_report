mod args;
mod commands;
use clap::Parser;
use rpx_runtime::error::{envelope, Error};
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
                print!("{e}");
                return;
            }
            let (_, value) = envelope(
                "arguments",
                Err(Error::new(2, "USAGE", "arguments", e)),
                started.elapsed().as_millis(),
            );
            println!("{value}");
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
        println!("{value}");
    }
    std::process::exit(code);
}
