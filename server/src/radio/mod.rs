pub mod hamlib;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use hamlib::RigCtld;
use crate::state::{AppState, RadioStatus};

/// Commands sent to the radio task from the timing engine and WebSocket handler.
pub enum RadioCommand {
    SetPtt(bool),
    SetFrequency(u64),
    SetMode(String, i32),
}

/// Radio polling and command task.  Owns the rigctld connection exclusively —
/// no shared mutex required.  PTT, frequency, and mode commands arrive via
/// `cmd_rx` and are executed immediately between polls.
pub async fn run(state: Arc<AppState>, mut cmd_rx: mpsc::Receiver<RadioCommand>) {
    let (rig_host, rig_port, poll_interval_ms) = {
        let cfg = state.config.read().unwrap();
        (
            cfg.radio.rigctld_host.clone(),
            cfg.radio.rigctld_port,
            cfg.radio.poll_interval_ms,
        )
    };

    'outer: loop {
        // ---- Connect --------------------------------------------------------
        let mut rig = loop {
            match RigCtld::connect(&rig_host, rig_port).await {
                Ok(rig) => {
                    tracing::info!("Connected to rigctld at {}:{}", rig_host, rig_port);
                    break rig;
                }
                Err(e) => {
                    tracing::warn!("rigctld not available: {e}");
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
                        tracing::warn!("rigctld command failed: {e} — reconnecting");
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
                            tracing::warn!("rigctld poll error: {e} — reconnecting");
                            let _ = state.radio_tx.send(RadioStatus::default());
                            continue 'outer;
                        }
                    }
                }
            }
        }
    }
}

async fn poll_once(rig: &mut RigCtld) -> anyhow::Result<RadioStatus> {
    let freq = rig.get_frequency().await?;
    let (mode, _) = rig.get_mode().await?;
    let ptt = rig.get_ptt().await?;
    Ok(RadioStatus { connected: true, freq, mode, ptt })
}
