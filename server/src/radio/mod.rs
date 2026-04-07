pub mod hamlib;

use std::sync::Arc;
use std::time::Duration;

use hamlib::RigCtld;

use crate::state::{AppState, RadioStatus};

pub async fn run(state: Arc<AppState>) {
    // Copy radio config values eagerly — changes to config require restart to take effect.
    let (rig_host, rig_port, poll_interval_ms) = {
        let cfg = state.config.read().unwrap();
        (
            cfg.radio.rigctld_host.clone(),
            cfg.radio.rigctld_port,
            cfg.radio.poll_interval_ms,
        )
    };

    loop {
        match RigCtld::connect(&rig_host, rig_port).await {
            Err(e) => {
                tracing::warn!("rigctld not available: {}", e);
                let _ = state.radio_tx.send(RadioStatus::default());
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Ok(rig) => {
                tracing::info!(
                    "Connected to rigctld at {}:{}",
                    rig_host,
                    rig_port
                );
                *state.rig.lock().await = Some(rig);

                loop {
                    let poll_result = {
                        let mut guard = state.rig.lock().await;
                        if let Some(rig) = guard.as_mut() {
                            poll_once(rig).await
                        } else {
                            break;
                        }
                    };

                    match poll_result {
                        Ok(status) => {
                            *state.last_radio_status.lock().await = status.clone();
                            let _ = state.radio_tx.send(status);
                        }
                        Err(e) => {
                            tracing::warn!("rigctld poll error: {e}");
                            *state.rig.lock().await = None;
                            break;
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
                }

                let _ = state.radio_tx.send(RadioStatus::default());
            }
        }
    }
}

async fn poll_once(rig: &mut RigCtld) -> anyhow::Result<RadioStatus> {
    let freq = rig.get_frequency().await?;
    let (mode, _) = rig.get_mode().await?;
    let ptt = rig.get_ptt().await?;
    Ok(RadioStatus {
        connected: true,
        freq,
        mode,
        ptt,
    })
}
