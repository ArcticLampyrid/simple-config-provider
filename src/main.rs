use simple_config_provider::{config, server::router};
use std::{env, path::PathBuf};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_writer(std::io::stderr)
        .init();
    let config_dir = std::path::absolute(PathBuf::from(
        env::var_os("CONFIG_DIR").unwrap_or_else(|| "config".into()),
    ))?;
    // Fail fast on an invalid configuration; requests reload it afterwards.
    config::load(&config_dir).await?;
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "3000".into()).parse()?;
    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    tracing::info!(address = %listener.local_addr()?, "simple-config-provider listening");
    axum::serve(listener, router(config_dir))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
