use std::net::SocketAddr;
use std::sync::atomic::Ordering;

use axum::{
    extract::{ConnectInfo, State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, mpsc};

use crate::engine::qso::{self, QsoState};
use crate::state::{QsoUpdate, SharedState, TxRequest};
use crate::web::session::ClientId;
use super::messages::{ClientMessage, DecodedMessageJson, ServerMessage};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, addr.to_string()))
}

async fn handle_socket(socket: WebSocket, state: SharedState, remote_addr: String) {
    let (per_client_tx, mut per_client_rx) = mpsc::channel::<ServerMessage>(64);
    let client_id = state.sessions.connect(remote_addr, per_client_tx).await;
    tracing::info!(client_id = %client_id, "WebSocket connected");

    let (mut sender, mut receiver) = socket.split();
    let mut waterfall_rx = state.waterfall_tx.subscribe();
    let mut decode_rx    = state.decode_tx.subscribe();
    let mut radio_rx     = state.radio_tx.subscribe();
    let mut qso_rx       = state.qso_tx.subscribe();

    // Local auth flag — avoids per-message session lookups for the hot broadcast path
    let mut authenticated = !state.sessions.needs_viewer_auth();

    // Send Hello so the client knows whether to show the viewer password form
    let hello = ServerMessage::Hello { needs_viewer_auth: state.sessions.needs_viewer_auth() };
    if send_msg(&mut sender, &hello).await.is_err() {
        state.sessions.disconnect(client_id).await;
        return;
    }

    // If no viewer auth required, send full initial state immediately
    if authenticated {
        send_initial_state(&state, &mut sender, client_id).await;
        state.sessions.broadcast_operator_status().await;
    }

    loop {
        tokio::select! {
            // Per-client targeted messages (OperatorStatus, AuthResult, etc.)
            msg = per_client_rx.recv() => {
                match msg {
                    Some(msg) => {
                        if send_msg(&mut sender, &msg).await.is_err() { break; }
                    }
                    None => break,
                }
            }

            // Incoming client messages
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let became_authed = handle_client_message(
                            &text, &state, &mut sender, client_id, authenticated,
                        ).await.unwrap_or(false);
                        if became_authed {
                            authenticated = true;
                            send_initial_state(&state, &mut sender, client_id).await;
                            state.sessions.broadcast_operator_status().await;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }

            // Broadcast subscriptions — only forwarded to authenticated clients
            line = waterfall_rx.recv(), if authenticated => {
                match line {
                    Ok(line) => {
                        let msg = ServerMessage::Waterfall {
                            timestamp: line.timestamp,
                            freq_min:  line.freq_min,
                            freq_max:  line.freq_max,
                            data:      line.data_b64,
                        };
                        if send_msg(&mut sender, &msg).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(client_id = %client_id, "lagged by {n} waterfall lines");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            result = decode_rx.recv(), if authenticated => {
                match result {
                    Ok(result) => {
                        let messages: Vec<DecodedMessageJson> = result.messages.iter().map(|m| {
                            DecodedMessageJson { snr: m.snr, dt: m.dt, freq: m.freq, message: m.message.clone() }
                        }).collect();
                        let msg = ServerMessage::Decode { period: result.period, messages };
                        if send_msg(&mut sender, &msg).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(client_id = %client_id, "lagged by {n} decode results");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            status = radio_rx.recv(), if authenticated => {
                match status {
                    Ok(status) => {
                        let msg = ServerMessage::RadioStatus {
                            connected: status.connected,
                            freq:      status.freq,
                            mode:      status.mode,
                            ptt:       status.ptt,
                        };
                        if send_msg(&mut sender, &msg).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(client_id = %client_id, "lagged by {n} radio status updates");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            update = qso_rx.recv(), if authenticated => {
                match update {
                    Ok(update) => {
                        let msg = ServerMessage::QsoUpdate {
                            state:      serde_json::to_value(update.state).unwrap_or_default(),
                            next_tx:    update.next_tx,
                            tx_enabled: update.tx_enabled,
                            tx_queued:  update.tx_queued,
                        };
                        if send_msg(&mut sender, &msg).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(client_id = %client_id, "lagged by {n} QSO updates");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    state.sessions.disconnect(client_id).await;
    state.sessions.broadcast_operator_status().await;
    tracing::info!(client_id = %client_id, "WebSocket disconnected");
}

/// Send current full state to a newly (re)authenticated client.
async fn send_initial_state(
    state: &SharedState,
    sender: &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
    client_id: ClientId,
) {
    // Radio status
    {
        let s = state.last_radio_status.lock().await;
        let msg = ServerMessage::RadioStatus {
            connected: s.connected,
            freq:      s.freq,
            mode:      s.mode.clone(),
            ptt:       s.ptt,
        };
        let _ = send_msg(sender, &msg).await;
    }

    // QSO state
    {
        let qso   = state.qso.lock().await;
        let guard = state.tx_queue.lock().await;
        let msg = ServerMessage::QsoUpdate {
            state:      serde_json::to_value(qso.clone()).unwrap_or_default(),
            next_tx:    guard.as_ref().map(|r| r.message.clone()),
            tx_enabled: state.tx_enabled.load(Ordering::Relaxed),
            tx_queued:  guard.is_some(),
        };
        let _ = send_msg(sender, &msg).await;
    }

    // Recent decodes (oldest first so client renders them in time order)
    {
        let cache = state.recent_decodes.lock().await;
        for period in cache.iter().rev() {
            let messages: Vec<DecodedMessageJson> = period.messages.iter().map(|m| {
                DecodedMessageJson { snr: m.snr, dt: m.dt, freq: m.freq, message: m.message.clone() }
            }).collect();
            let msg = ServerMessage::Decode { period: period.period, messages };
            let _ = send_msg(sender, &msg).await;
        }
    }

    // Operator status (personalised for this client)
    {
        let op_id = state.sessions.current_operator().await;
        let count = state.sessions.client_count().await;
        let msg = ServerMessage::OperatorStatus {
            operator_client_id: op_id.map(|id| id.to_string()),
            you_are_operator:   op_id == Some(client_id),
            client_count:       count,
        };
        let _ = send_msg(sender, &msg).await;
    }
}

/// Returns `Ok(true)` when the client just became authenticated (so the caller
/// can trigger initial-state sync).
async fn handle_client_message(
    text:          &str,
    state:         &SharedState,
    sender:        &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
    client_id:     ClientId,
    authenticated: bool,
) -> anyhow::Result<bool> {
    // Unauthenticated clients may only send Auth
    if !authenticated {
        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Auth { password }) => {
                let ok = state.sessions.authenticate(client_id, &password).await;
                let reply = ServerMessage::AuthResult { success: ok };
                let _ = send_msg(sender, &reply).await;
                return Ok(ok);
            }
            _ => {
                let reply = ServerMessage::Error { message: "Authentication required".into() };
                let _ = send_msg(sender, &reply).await;
                return Ok(false);
            }
        }
    }

    // For operator-only commands, verify the lock before dispatching
    let is_operator_msg = |msg: &ClientMessage| matches!(msg,
        ClientMessage::CallCq { .. }
        | ClientMessage::RespondTo { .. }
        | ClientMessage::QueueTx { .. }
        | ClientMessage::HaltTx {}
        | ClientMessage::EnableTx { .. }
        | ClientMessage::SetTxParity { .. }
        | ClientMessage::SetFrequency { .. }
        | ClientMessage::SetMode { .. }
        | ClientMessage::ResetQso {}
    );

    match serde_json::from_str::<ClientMessage>(text) {
        Ok(msg) => {
            if is_operator_msg(&msg) && !state.sessions.is_operator(client_id).await {
                let reply = ServerMessage::Error { message: "Operator access required".into() };
                let _ = send_msg(sender, &reply).await;
                return Ok(false);
            }

            match msg {
                ClientMessage::Ping {} => {
                    let reply = ServerMessage::Echo { payload: serde_json::json!({ "pong": true }) };
                    let _ = send_msg(sender, &reply).await;
                }

                ClientMessage::Auth { .. } => {
                    // Already authenticated — no-op
                }

                ClientMessage::ClaimOperator { password } => {
                    let ok = state.sessions.claim_operator(client_id, &password).await;
                    if ok {
                        state.sessions.broadcast_operator_status().await;
                    } else {
                        let reply = ServerMessage::Error { message: "Wrong operator password".into() };
                        let _ = send_msg(sender, &reply).await;
                    }
                }

                ClientMessage::ReleaseOperator {} => {
                    state.sessions.release_operator(client_id).await;
                    state.sessions.broadcast_operator_status().await;
                }

                ClientMessage::SetFrequency { freq } => {
                    let mut guard = state.rig.lock().await;
                    if let Some(rig) = guard.as_mut() {
                        if let Err(e) = rig.set_frequency(freq).await {
                            tracing::warn!("set_frequency error: {e}");
                        }
                    }
                }

                ClientMessage::SetMode { mode, passband } => {
                    let mut guard = state.rig.lock().await;
                    if let Some(rig) = guard.as_mut() {
                        if let Err(e) = rig.set_mode(&mode, passband).await {
                            tracing::warn!("set_mode error: {e}");
                        }
                    }
                }

                ClientMessage::EnableTx { enabled } => {
                    state.tx_enabled.store(enabled, Ordering::Relaxed);
                    tracing::info!("TX {}", if enabled { "enabled" } else { "disabled" });
                    broadcast_qso_update(state).await;
                }

                ClientMessage::SetTxParity { parity } => {
                    state.desired_tx_parity.store(parity != 0, Ordering::Relaxed);
                    tracing::info!("TX parity set to {}", parity);
                }

                ClientMessage::CallCq { freq } => {
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
                            let _ = send_msg(sender, &reply).await;
                        }
                    }
                }

                ClientMessage::RespondTo { their_call, their_freq: _, tx_freq } => {
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
                            let _ = send_msg(sender, &reply).await;
                        }
                    }
                }

                ClientMessage::QueueTx { message, freq } => {
                    match encode_tx(&message, freq, state.tx_sample_rate).await {
                        Ok(samples) => {
                            *state.tx_queue.lock().await = Some(TxRequest { samples, message: message.clone() });
                            tracing::info!("Manual TX queued: {}", message);
                            broadcast_qso_update(state).await;
                        }
                        Err(e) => {
                            let reply = ServerMessage::Error { message: format!("encode error: {e}") };
                            let _ = send_msg(sender, &reply).await;
                        }
                    }
                }

                ClientMessage::HaltTx {} => {
                    state.tx_enabled.store(false, Ordering::Relaxed);
                    state.tx_queue.lock().await.take();
                    if let Some(pb) = state.playback.as_ref() {
                        pb.cancel();
                    }
                    let mut guard = state.rig.lock().await;
                    if let Some(rig) = guard.as_mut() {
                        if let Err(e) = rig.set_ptt(false).await {
                            tracing::warn!("PTT deassert on HaltTx failed: {e}");
                        }
                    }
                    tracing::info!("TX halted");
                    broadcast_qso_update(state).await;
                }

                ClientMessage::ResetQso {} => {
                    *state.qso.lock().await = QsoState::Idle;
                    state.tx_enabled.store(false, Ordering::Relaxed);
                    state.tx_queue.lock().await.take();
                    tracing::info!("QSO reset");
                    broadcast_qso_update(state).await;
                }
            }

            Ok(false)
        }

        Err(e) => {
            tracing::warn!(client_id = %client_id, error = %e, "unknown message");
            let reply = ServerMessage::Error { message: format!("unknown message: {e}") };
            let _ = send_msg(sender, &reply).await;
            Ok(false)
        }
    }
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

/// Push a fresh QSO update to all subscribers via the broadcast channel.
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

/// Serialise and send a ServerMessage; returns Err if the socket is closed.
async fn send_msg(
    sender: &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
    msg: &ServerMessage,
) -> Result<(), axum::Error> {
    let json = serde_json::to_string(msg).unwrap();
    sender.send(Message::Text(json.into())).await
}
