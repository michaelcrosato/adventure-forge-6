use forge_content::parse_and_compile_production;
use forge_server::http::router;
use std::{net::Ipv4Addr, process::ExitCode, sync::Arc};

#[tokio::main(worker_threads = 2)]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), &'static str> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let port = match arguments.as_slice() {
        [] => 38123,
        [flag, port] if flag == "--port" => port.parse::<u16>().map_err(|_| "Invalid port.")?,
        _ => return Err("Usage: forge-server [--port NUMBER]"),
    };
    let content = parse_and_compile_production(include_str!("../../../content/split-tide.json"))
        .map_err(|_| "Game content unavailable.")?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|_| "Local listener unavailable.")?;
    let port = listener
        .local_addr()
        .map_err(|_| "Local listener unavailable.")?
        .port();
    let app = router(Arc::new(content), port).map_err(|_| "Local service unavailable.")?;
    println!("Adventure Forge local API: http://127.0.0.1:{port}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| "Local service stopped unexpectedly.")
}
