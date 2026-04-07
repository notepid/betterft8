use std::sync::Arc;
use anyhow::Result;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod audio;
mod config;
mod dsp;
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

    let (waterfall_tx, _) = broadcast::channel(32);

    // Start audio capture on the current (non-async) context before spawning tasks.
    // The _stream must stay alive for the duration of the program.
    let (ring_consumer, effective_rate, _stream) =
        audio::capture::start_capture(&config.audio)?;

    let state = Arc::new(AppState {
        config,
        waterfall_tx: waterfall_tx.clone(),
    });

    // Spawn DSP waterfall task
    tokio::spawn(dsp::waterfall::run(ring_consumer, effective_rate, waterfall_tx));

    let router = build_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
