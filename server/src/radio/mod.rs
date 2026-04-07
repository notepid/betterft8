pub mod hamlib;

use std::sync::Arc;
use std::time::Duration;

use hamlib::RigCtld;

use crate::state::{AppState, RadioStatus};

pub async fn run(state: Arc<AppState>) {
    let config = &state.config.radio;

    loop {
        match RigCtld::connect(&config.rigctld_host, config.rigctld_port).await {
            Err(e) => {
                tracing::warn!("rigctld not available: {}", e);
                let _ = state.radio_tx.send(RadioStatus::default());
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Ok(rig) => {
                tracing::info!(
                    "Connected to rigctld at {}:{}",
                    config.rigctld_host,
                    config.rigctld_port
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
                            let _ = state.radio_tx.send(status);
                        }
                        Err(e) => {
                            tracing::warn!("rigctld poll error: {e}");
                            *state.rig.lock().await = None;
                            break;
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(config.poll_interval_ms)).await;
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
