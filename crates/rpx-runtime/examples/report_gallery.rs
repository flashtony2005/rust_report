//! Agent 通过本地进程执行引擎；展示页只读取本命令产生的固定产物。
use std::{path::Path, process::ExitCode};

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        eprintln!("usage: report_gallery <source-dir> <output-dir> [datasources.json]");
        return ExitCode::from(2);
    }
    match rpx_runtime::gallery::generate(
        Path::new(&args[0]),
        Path::new(&args[1]),
        args.get(2).map(Path::new),
    )
    .await
    {
        Ok(catalog) => {
            println!(
                "{}",
                serde_json::json!({"ok":true,"run_id":catalog["run_id"],"reports":catalog["reports"].as_array().map(Vec::len),"catalog":Path::new(&args[1]).join("catalog.json")})
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
