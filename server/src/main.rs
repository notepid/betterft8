use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod audio;
mod config;
mod dsp;
mod engine;
mod radio;
mod state;
mod web;

use state::{AppState, QsoUpdate};
use web::server::build_router;
use web::session::SessionManager;

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
    let (decode_tx, _)    = broadcast::channel(16);
    let (radio_tx, _)     = broadcast::channel(8);
    let (qso_tx, _)       = broadcast::channel::<QsoUpdate>(16);

    // Shared rolling audio buffer for the FT8 decode engine.
    let decode_buf = Arc::new(Mutex::new(Vec::<f32>::new()));

    // Audio input — _in_stream must stay alive for the duration of the program.
    let (ring_consumer, effective_rate, _in_stream) =
        audio::capture::start_capture(&config.audio, decode_buf.clone())?;

    // Audio output — _out_stream must stay alive; PlaybackHandle goes into AppState.
    let (playback, tx_sample_rate, _out_stream) =
        match audio::playback::start_playback() {
            Ok((handle, stream)) => {
                tracing::info!("Audio output ready at {}Hz", handle.sample_rate);
                let rate = handle.sample_rate;
                (Some(handle), rate, Some(stream))
            }
            Err(e) => {
                tracing::warn!("Audio output unavailable (TX will be PTT-only): {e}");
                (None, 12_000u32, None)
            }
        };

    let sessions = SessionManager::new(
        config.network.operator_password.clone(),
        config.network.viewer_password.clone(),
    );

    let state = Arc::new(AppState {
        config,
        sessions,
        waterfall_tx: waterfall_tx.clone(),
        decode_tx:    decode_tx.clone(),
        radio_tx:     radio_tx.clone(),
        qso_tx:       qso_tx.clone(),
        recent_decodes:    tokio::sync::Mutex::new(VecDeque::new()),
        last_radio_status: tokio::sync::Mutex::new(state::RadioStatus::default()),
        rig:          tokio::sync::Mutex::new(None),
        tx_queue:     tokio::sync::Mutex::new(None),
        tx_enabled:   AtomicBool::new(false),
        desired_tx_parity: AtomicBool::new(false),
        qso:          tokio::sync::Mutex::new(engine::qso::QsoState::Idle),
        playback,
        tx_sample_rate,
    });

    // Spawn DSP waterfall task
    tokio::spawn(dsp::waterfall::run(ring_consumer, effective_rate, waterfall_tx));

    // Spawn FT8 decode + TX timing engine
    tokio::spawn(engine::timing::run(state.clone(), decode_buf, effective_rate));

    // Spawn radio polling task
    tokio::spawn(radio::run(state.clone()));

    let router = build_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Keep audio streams alive until the server exits.
    let _keep_alive = (_in_stream, _out_stream);

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    ).await?;

    Ok(())
}
