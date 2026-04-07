pub mod hamlib;
#[cfg(feature = "hamlib")]
mod hamlib_ffi;
#[cfg(feature = "hamlib")]
pub mod hamlib_direct;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::state::{AppState, RadioStatus};

/// Trait abstracting radio control — implemented by both the rigctld TCP client
/// and the direct Hamlib FFI backend.
pub trait RadioBackend: Send {
    async fn get_frequency(&mut self) -> Result<u64>;
    async fn set_frequency(&mut self, freq: u64) -> Result<()>;
    async fn get_mode(&mut self) -> Result<(String, i32)>;
    async fn set_mode(&mut self, mode: &str, passband: i32) -> Result<()>;
    async fn get_ptt(&mut self) -> Result<bool>;
    async fn set_ptt(&mut self, on: bool) -> Result<()>;
}

/// Enum dispatch for radio backends — avoids dyn trait object-safety issues
/// with async fn in traits.
enum Backend {
    RigCtld(hamlib::RigCtld),
    #[cfg(feature = "hamlib")]
    Hamlib(hamlib_direct::HamlibDirect),
}

impl Backend {
    async fn get_frequency(&mut self) -> Result<u64> {
        match self {
            Backend::RigCtld(r) => r.get_frequency().await,
            #[cfg(feature = "hamlib")]
            Backend::Hamlib(r) => r.get_frequency().await,
        }
    }
    async fn set_frequency(&mut self, freq: u64) -> Result<()> {
        match self {
            Backend::RigCtld(r) => r.set_frequency(freq).await,
            #[cfg(feature = "hamlib")]
            Backend::Hamlib(r) => r.set_frequency(freq).await,
        }
    }
    async fn get_mode(&mut self) -> Result<(String, i32)> {
        match self {
            Backend::RigCtld(r) => r.get_mode().await,
            #[cfg(feature = "hamlib")]
            Backend::Hamlib(r) => r.get_mode().await,
        }
    }
    async fn set_mode(&mut self, mode: &str, passband: i32) -> Result<()> {
        match self {
            Backend::RigCtld(r) => r.set_mode(mode, passband).await,
            #[cfg(feature = "hamlib")]
            Backend::Hamlib(r) => r.set_mode(mode, passband).await,
        }
    }
    async fn get_ptt(&mut self) -> Result<bool> {
        match self {
            Backend::RigCtld(r) => r.get_ptt().await,
            #[cfg(feature = "hamlib")]
            Backend::Hamlib(r) => r.get_ptt().await,
        }
    }
    async fn set_ptt(&mut self, on: bool) -> Result<()> {
        match self {
            Backend::RigCtld(r) => r.set_ptt(on).await,
            #[cfg(feature = "hamlib")]
            Backend::Hamlib(r) => r.set_ptt(on).await,
        }
    }
}

/// Commands sent to the radio task from the timing engine and WebSocket handler.
pub enum RadioCommand {
    SetPtt(bool),
    SetFrequency(u64),
    SetMode(String, i32),
}

/// Create the appropriate radio backend based on config.
async fn create_backend(state: &AppState) -> Result<Backend> {
    let cfg = state.config.read().unwrap().radio.clone();
    match cfg.backend.as_str() {
        #[cfg(feature = "hamlib")]
        "hamlib" => {
            let model = cfg.rig_model.ok_or_else(|| anyhow::anyhow!("rig_model required for hamlib backend"))?;
            let port = cfg.serial_port.clone().ok_or_else(|| anyhow::anyhow!("serial_port required for hamlib backend"))?;
            let baud = cfg.baud_rate.unwrap_or(9600);
            let backend = tokio::task::spawn_blocking(move || {
                hamlib_direct::HamlibDirect::new(model, &port, baud)
            }).await??;
            tracing::info!("Hamlib direct backend connected (model={}, port={})",
                cfg.rig_model.unwrap(), cfg.serial_port.as_deref().unwrap());
            Ok(Backend::Hamlib(backend))
        }
        #[cfg(not(feature = "hamlib"))]
        "hamlib" => {
            anyhow::bail!("hamlib backend not available — compiled without 'hamlib' feature");
        }
        _ => {
            let rig = hamlib::RigCtld::connect(&cfg.rigctld_host, cfg.rigctld_port).await?;
            tracing::info!("Connected to rigctld at {}:{}", cfg.rigctld_host, cfg.rigctld_port);
            Ok(Backend::RigCtld(rig))
        }
    }
}

/// Radio polling and command task.  Owns the radio connection exclusively —
/// no shared mutex required.  PTT, frequency, and mode commands arrive via
/// `cmd_rx` and are executed immediately between polls.
pub async fn run(state: Arc<AppState>, mut cmd_rx: mpsc::Receiver<RadioCommand>) {
    let poll_interval_ms = {
        let cfg = state.config.read().unwrap();
        cfg.radio.poll_interval_ms
    };

    'outer: loop {
        // ---- Connect --------------------------------------------------------
        let mut rig: Backend = loop {
            match create_backend(&state).await {
                Ok(backend) => break backend,
                Err(e) => {
                    tracing::warn!("Radio backend not available: {e}");
                    let _ = state.radio_tx.send(RadioStatus::default());

                    // Wait 10 s before retrying; drain commands so senders don't block.
                    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
                    loop {
                        match tokio::time::timeout_at(deadline, cmd_rx.recv()).await {
                            Ok(None) => return, // channel closed — shut down
                            Ok(Some(_)) => {}   // discard; rig not connected
                            Err(_) => break,    // 10 s elapsed — retry connect
                        }
                    }
                }
            }
        };

        // ---- Poll + command loop --------------------------------------------
        let mut poll_timer =
            tokio::time::interval(Duration::from_millis(poll_interval_ms));
        poll_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                biased; // Commands (PTT!) have priority over polling.

                cmd = cmd_rx.recv() => {
                    let cmd = match cmd {
                        Some(c) => c,
                        None    => return, // channel closed — shut down
                    };
                    let result = match cmd {
                        RadioCommand::SetPtt(on) => {
                            let r = rig.set_ptt(on).await;
                            if r.is_ok() {
                                tracing::info!("PTT {}", if on { "ON" } else { "OFF" });
                            }
                            r
                        }
                        RadioCommand::SetFrequency(freq) => rig.set_frequency(freq).await,
                        RadioCommand::SetMode(mode, passband) => rig.set_mode(&mode, passband).await,
                    };
                    if let Err(e) = result {
                        tracing::warn!("Radio command failed: {e} — reconnecting");
                        let _ = state.radio_tx.send(RadioStatus::default());
                        continue 'outer;
                    }
                }

                _ = poll_timer.tick() => {
                    match poll_once(&mut rig).await {
                        Ok(status) => {
                            *state.last_radio_status.lock().await = status.clone();
                            let _ = state.radio_tx.send(status);
                        }
                        Err(e) => {
                            tracing::warn!("Radio poll error: {e} — reconnecting");
                            let _ = state.radio_tx.send(RadioStatus::default());
                            continue 'outer;
                        }
                    }
                }
            }
        }
    }
}

async fn poll_once(rig: &mut Backend) -> anyhow::Result<RadioStatus> {
    let freq = rig.get_frequency().await?;
    let (mode, _) = rig.get_mode().await?;
    let ptt = rig.get_ptt().await?;
    Ok(RadioStatus { connected: true, freq, mode, ptt })
}
