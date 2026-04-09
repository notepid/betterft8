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

use state::{AppState, LogEntryData, QsoUpdate};
use web::server::build_router;
use web::session::SessionManager;
use radio::RadioCommand;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let setup_mode = !config::config_file_exists();
    let config = config::load()?;

    let addr = format!("{}:{}", config.network.host, config.network.port);
    tracing::info!("BetterFT8 server starting on {addr}");

    let (waterfall_tx, _)  = broadcast::channel(32);
    let (decode_tx, _)     = broadcast::channel(16);
    let (radio_tx, _)      = broadcast::channel(8);
    let (qso_tx, _)        = broadcast::channel::<QsoUpdate>(16);
    let (log_tx, _)        = broadcast::channel::<LogEntryData>(8);
    let (radio_cmd_tx, radio_cmd_rx) = tokio::sync::mpsc::channel::<RadioCommand>(16);

    // Shared rolling audio buffer for the FT8 decode engine.
    let decode_buf = Arc::new(Mutex::new(Vec::<f32>::new()));

    // Audio input — _in_stream must stay alive for the duration of the program.
    let (ring_consumer, effective_rate, _in_stream) =
        audio::capture::start_capture(&config.audio, decode_buf.clone())?;

    // Audio output — _out_stream must stay alive; PlaybackHandle goes into AppState.
    let (playback, tx_sample_rate, _out_stream) =
        match audio::playback::start_playback(config.audio.output_device.as_deref()) {
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

    // Enumerate audio devices for the Settings panel.
    let (audio_input_devices, audio_output_devices) = list_audio_devices();
    tracing::info!(
        "Audio devices: {} inputs, {} outputs",
        audio_input_devices.len(),
        audio_output_devices.len()
    );

    // TLS config (read before moving config into AppState).
    let tls_cert = config.network.tls_cert.clone();
    let tls_key  = config.network.tls_key.clone();

    let state = Arc::new(AppState {
        config: std::sync::RwLock::new(config),
        sessions,
        waterfall_tx: waterfall_tx.clone(),
        decode_tx:    decode_tx.clone(),
        radio_tx:     radio_tx.clone(),
        qso_tx:       qso_tx.clone(),
        log_tx:       log_tx.clone(),
        recent_decodes:    tokio::sync::Mutex::new(VecDeque::new()),
        last_radio_status: tokio::sync::Mutex::new(state::RadioStatus::default()),
        radio_cmd_tx,
        tx_queue:     tokio::sync::Mutex::new(None),
        tx_enabled:   AtomicBool::new(false),
        desired_tx_parity: AtomicBool::new(false),
        qso:          tokio::sync::Mutex::new(engine::qso::QsoState::Idle),
        qso_start:    Mutex::new(None),
        playback,
        tx_sample_rate,
        audio_input_devices,
        audio_output_devices,
        setup_mode:   AtomicBool::new(setup_mode),
        os_type:      detect_os(),
    });

    // Spawn DSP waterfall task
    tokio::spawn(dsp::waterfall::run(ring_consumer, effective_rate, waterfall_tx));

    // Spawn FT8 decode + TX timing engine
    tokio::spawn(engine::timing::run(state.clone(), decode_buf, effective_rate));

    // Spawn radio polling task
    tokio::spawn(radio::run(state.clone(), radio_cmd_rx));

    let router = build_router(state);

    // Keep audio streams alive until the server exits.
    let _keep_alive = (_in_stream, _out_stream);

    // Start HTTP or HTTPS server.
    if let (Some(cert), Some(key)) = (tls_cert, tls_key) {
        tracing::info!("TLS enabled — serving HTTPS/WSS on {addr}");
        let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
        axum_server::bind_rustls(addr.parse()?, tls_config)
            .serve(router.into_make_service_with_connect_info::<SocketAddr>())
            .await?;
    } else {
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        ).await?;
    }

    Ok(())
}

/// Detect the host OS, including Raspberry Pi as a special case.
fn detect_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        let is_rpi = std::fs::read_to_string("/proc/cpuinfo")
            .map(|s| s.contains("Raspberry Pi"))
            .unwrap_or(false);
        if is_rpi { "raspberry_pi" } else { "linux" }
    } else {
        "unknown"
    }
}

/// Enumerate cpal audio input and output device names.
fn list_audio_devices() -> (Vec<String>, Vec<String>) {
    use cpal::traits::HostTrait;
    let host = cpal::default_host();
    let inputs = host
        .input_devices()
        .map(|iter| {
            iter.map(|d| {
                use cpal::traits::DeviceTrait;
                d.name().unwrap_or_else(|_| "unknown".to_string())
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let outputs = host
        .output_devices()
        .map(|iter| {
            iter.map(|d| {
                use cpal::traits::DeviceTrait;
                d.name().unwrap_or_else(|_| "unknown".to_string())
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (inputs, outputs)
}
