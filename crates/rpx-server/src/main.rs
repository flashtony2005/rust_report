use std::{net::SocketAddr, path::PathBuf};
#[tokio::main]
async fn main() {
    if let Err(error) = start().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
async fn start() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut addr = "127.0.0.1:8090".parse::<SocketAddr>()?;
    let mut library = None;
    let mut auth_token = std::env::var("RPX_AUTH_TOKEN").ok();
    if args.next().as_deref() != Some("serve") {
        return Err(
            "用法：rpx-server serve --library DIR [--addr 127.0.0.1:8090] [--auth-token TOKEN]"
                .into(),
        );
    }
    while let Some(key) = args.next() {
        match key.as_str() {
            "--addr" => addr = args.next().ok_or("缺少 --addr")?.parse()?,
            "--library" => library = Some(PathBuf::from(args.next().ok_or("缺少 --library")?)),
            "--auth-token" => {
                auth_token = Some(args.next().ok_or("缺少 --auth-token 值")?);
            }
            _ => return Err(format!("未知选项 {key}").into()),
        }
    }
    let library = library.ok_or("必须指定 --library DIR")?;
    rpx_runtime::library::list(&library, None, None, 1, 0)?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("报表库 {} → http://{addr}", library.display());
    if auth_token.is_some() {
        eprintln!("Bearer 鉴权已启用");
    }
    axum::serve(
        listener,
        rpx_server::http::app_with_auth(library, auth_token),
    )
    .await?;
    Ok(())
}
