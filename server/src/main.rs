use std::sync::Arc;
use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod state;
mod web;

use state::AppState;
use web::server::build_router;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = config::load()?;

    let addr = format!("{}:{}", config.network.host, config.network.port);
    tracing::info!("BetterFT8 server starting on {addr}");

    let state = Arc::new(AppState { config });
    let router = build_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
