use std::sync::atomic::Ordering;

use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::engine::qso::{self, QsoState};
use crate::state::{QsoUpdate, SharedState, TxRequest};
use super::messages::{ClientMessage, DecodedMessageJson, ServerMessage};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: SharedState) {
    let conn_id = Uuid::new_v4();
    tracing::info!(conn_id = %conn_id, "WebSocket connected");

    let (mut sender, mut receiver) = socket.split();
    let mut waterfall_rx = state.waterfall_tx.subscribe();
    let mut decode_rx    = state.decode_tx.subscribe();
    let mut radio_rx     = state.radio_tx.subscribe();
    let mut qso_rx       = state.qso_tx.subscribe();

    // Send initial QSO state on connect
    {
        let qso   = state.qso.lock().await;
        let guard = state.tx_queue.lock().await;
        let update = ServerMessage::QsoUpdate {
            state:      serde_json::to_value(qso.clone()).unwrap_or_default(),
            next_tx:    guard.as_ref().map(|r| r.message.clone()),
            tx_enabled: state.tx_enabled.load(Ordering::Relaxed),
            tx_queued:  guard.is_some(),
        };
        let json = serde_json::to_string(&update).unwrap();
        let _ = sender.send(Message::Text(json.into())).await;
    }

    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_client_message(&text, &state, &mut sender, conn_id).await {
                            tracing::warn!(conn_id = %conn_id, "client message error: {e}");
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }

            line = waterfall_rx.recv() => {
                match line {
                    Ok(line) => {
                        let msg = ServerMessage::Waterfall {
                            timestamp: line.timestamp,
                            freq_min:  line.freq_min,
                            freq_max:  line.freq_max,
                            data:      line.data_b64,
                        };
                        let json = serde_json::to_string(&msg).unwrap();
                        if sender.send(Message::Text(json.into())).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(conn_id = %conn_id, "lagged by {n} waterfall lines");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            result = decode_rx.recv() => {
                match result {
                    Ok(result) => {
                        let messages: Vec<DecodedMessageJson> = result.messages.iter().map(|m| {
                            DecodedMessageJson { snr: m.snr, dt: m.dt, freq: m.freq, message: m.message.clone() }
                        }).collect();
                        let msg = ServerMessage::Decode { period: result.period, messages };
                        let json = serde_json::to_string(&msg).unwrap();
                        if sender.send(Message::Text(json.into())).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(conn_id = %conn_id, "lagged by {n} decode results");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            status = radio_rx.recv() => {
                match status {
                    Ok(status) => {
                        let msg = ServerMessage::RadioStatus {
                            connected: status.connected,
                            freq:      status.freq,
                            mode:      status.mode,
                            ptt:       status.ptt,
                        };
                        let json = serde_json::to_string(&msg).unwrap();
                        if sender.send(Message::Text(json.into())).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(conn_id = %conn_id, "lagged by {n} radio status updates");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            update = qso_rx.recv() => {
                match update {
                    Ok(update) => {
                        let msg = ServerMessage::QsoUpdate {
                            state:      serde_json::to_value(update.state).unwrap_or_default(),
                            next_tx:    update.next_tx,
                            tx_enabled: update.tx_enabled,
                            tx_queued:  update.tx_queued,
                        };
                        let json = serde_json::to_string(&msg).unwrap();
                        if sender.send(Message::Text(json.into())).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(conn_id = %conn_id, "lagged by {n} QSO updates");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    tracing::info!(conn_id = %conn_id, "WebSocket disconnected");
}

async fn handle_client_message(
    text:     &str,
    state:    &SharedState,
    sender:   &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
    conn_id:  uuid::Uuid,
) -> anyhow::Result<()> {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(ClientMessage::Ping {}) => {
            tracing::debug!(conn_id = %conn_id, "ping");
            let reply = ServerMessage::Echo { payload: serde_json::json!({ "pong": true }) };
            let json = serde_json::to_string(&reply).unwrap();
            let _ = sender.send(Message::Text(json.into())).await;
        }

        Ok(ClientMessage::SetFrequency { freq }) => {
            let mut guard = state.rig.lock().await;
            if let Some(rig) = guard.as_mut() {
                if let Err(e) = rig.set_frequency(freq).await {
                    tracing::warn!("set_frequency error: {e}");
                }
            }
        }

        Ok(ClientMessage::SetMode { mode, passband }) => {
            let mut guard = state.rig.lock().await;
            if let Some(rig) = guard.as_mut() {
                if let Err(e) = rig.set_mode(&mode, passband).await {
                    tracing::warn!("set_mode error: {e}");
                }
            }
        }

        Ok(ClientMessage::EnableTx { enabled }) => {
            state.tx_enabled.store(enabled, Ordering::Relaxed);
            tracing::info!("TX {}", if enabled { "enabled" } else { "disabled" });
            broadcast_qso_update(state).await;
        }

        Ok(ClientMessage::SetTxParity { parity }) => {
            state.desired_tx_parity.store(parity != 0, Ordering::Relaxed);
            tracing::info!("TX parity set to {}", parity);
        }

        Ok(ClientMessage::CallCq { freq }) => {
            let my_call = state.config.station.callsign.clone();
            let my_grid = state.config.station.grid.clone();
            let msg_text = qso::cq_message(&my_call, &my_grid);

            match encode_tx(&msg_text, freq, state.tx_sample_rate).await {
                Ok(samples) => {
                    *state.qso.lock().await = QsoState::CallingCq {
                        my_call: my_call.clone(),
                        my_grid,
                        tx_freq: freq,
                    };
                    *state.tx_queue.lock().await = Some(TxRequest { samples, message: msg_text.clone() });
                    state.tx_enabled.store(true, Ordering::Relaxed);
                    tracing::info!("CallCQ queued: {}", msg_text);
                    broadcast_qso_update(state).await;
                }
                Err(e) => {
                    tracing::error!("FT8 encode error for CQ: {e}");
                    let reply = ServerMessage::Error { message: format!("encode error: {e}") };
                    let json = serde_json::to_string(&reply).unwrap();
                    let _ = sender.send(Message::Text(json.into())).await;
                }
            }
        }

        Ok(ClientMessage::RespondTo { their_call, their_freq: _, tx_freq }) => {
            let my_call = state.config.station.callsign.clone();
            let my_grid = state.config.station.grid.clone();
            let msg_text = qso::grid_response(&their_call, &my_call, &my_grid);

            match encode_tx(&msg_text, tx_freq, state.tx_sample_rate).await {
                Ok(samples) => {
                    *state.qso.lock().await = QsoState::InQso {
                        their_call:   their_call.clone(),
                        their_grid:   None,
                        their_report: None,
                        my_report:    None,
                        my_grid:      Some(my_grid),
                        step:         crate::engine::qso::QsoStep::SentGrid,
                        tx_freq,
                    };
                    *state.tx_queue.lock().await = Some(TxRequest { samples, message: msg_text.clone() });
                    state.tx_enabled.store(true, Ordering::Relaxed);
                    tracing::info!("RespondTo {} queued: {}", their_call, msg_text);
                    broadcast_qso_update(state).await;
                }
                Err(e) => {
                    tracing::error!("FT8 encode error: {e}");
                    let reply = ServerMessage::Error { message: format!("encode error: {e}") };
                    let json = serde_json::to_string(&reply).unwrap();
                    let _ = sender.send(Message::Text(json.into())).await;
                }
            }
        }

        Ok(ClientMessage::QueueTx { message, freq }) => {
            match encode_tx(&message, freq, state.tx_sample_rate).await {
                Ok(samples) => {
                    *state.tx_queue.lock().await = Some(TxRequest { samples, message: message.clone() });
                    tracing::info!("Manual TX queued: {}", message);
                    broadcast_qso_update(state).await;
                }
                Err(e) => {
                    let reply = ServerMessage::Error { message: format!("encode error: {e}") };
                    let json = serde_json::to_string(&reply).unwrap();
                    let _ = sender.send(Message::Text(json.into())).await;
                }
            }
        }

        Ok(ClientMessage::HaltTx {}) => {
            state.tx_enabled.store(false, Ordering::Relaxed);
            state.tx_queue.lock().await.take();
            if let Some(pb) = state.playback.as_ref() {
                pb.cancel();
            }
            // Best-effort PTT deassert
            let mut guard = state.rig.lock().await;
            if let Some(rig) = guard.as_mut() {
                if let Err(e) = rig.set_ptt(false).await {
                    tracing::warn!("PTT deassert on HaltTx failed: {e}");
                }
            }
            tracing::info!("TX halted");
            broadcast_qso_update(state).await;
        }

        Ok(ClientMessage::ResetQso {}) => {
            *state.qso.lock().await = QsoState::Idle;
            state.tx_enabled.store(false, Ordering::Relaxed);
            state.tx_queue.lock().await.take();
            tracing::info!("QSO reset");
            broadcast_qso_update(state).await;
        }

        Err(e) => {
            tracing::warn!(conn_id = %conn_id, error = %e, "unknown message");
            let reply = ServerMessage::Error { message: format!("unknown message: {e}") };
            let json = serde_json::to_string(&reply).unwrap();
            let _ = sender.send(Message::Text(json.into())).await;
        }
    }

    Ok(())
}

/// Encode FT8 audio in a blocking thread pool.
async fn encode_tx(message: &str, freq: f32, sample_rate: u32) -> anyhow::Result<Vec<f32>> {
    let message = message.to_string();
    tokio::task::spawn_blocking(move || {
        crate::dsp::ft8::encode(&message, freq, sample_rate)
    })
    .await
    .map_err(|e| anyhow::anyhow!("spawn_blocking: {e}"))?
}

/// Push a fresh QSO update to all subscribers.
async fn broadcast_qso_update(state: &SharedState) {
    let qso   = state.qso.lock().await;
    let guard = state.tx_queue.lock().await;
    let update = QsoUpdate {
        state:      qso.clone(),
        next_tx:    guard.as_ref().map(|r| r.message.clone()),
        tx_enabled: state.tx_enabled.load(Ordering::Relaxed),
        tx_queued:  guard.is_some(),
    };
    let _ = state.qso_tx.send(update);
}
