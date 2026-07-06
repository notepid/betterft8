use std::net::SocketAddr;
use std::sync::atomic::Ordering;

use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{ConnectInfo, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, mpsc};

use super::messages::{ClientMessage, DecodedMessageJson, ServerMessage};
use crate::engine::qso::{self, QsoState};
use crate::radio::RadioCommand;
use crate::state::{LogEntryData, QsoUpdate, SharedState, TxRequest};
use crate::web::session::ClientId;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    // Guard against cross-site WebSocket hijacking: browsers send an `Origin`
    // header on WS upgrades, so reject any upgrade whose Origin is not
    // same-origin (matches the request `Host`) or a localhost address. Requests
    // with no Origin header are non-browser clients and are allowed.
    if !origin_allowed(&headers) {
        tracing::warn!(
            "Rejected WebSocket upgrade with disallowed Origin: {:?}",
            headers.get(axum::http::header::ORIGIN)
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state, addr.to_string()))
}

/// Returns true if the WS upgrade Origin is acceptable (same-origin, localhost,
/// or absent). Guards against cross-site WebSocket hijacking.
fn origin_allowed(headers: &HeaderMap) -> bool {
    use axum::http::header::{HOST, ORIGIN};

    let origin = match headers.get(ORIGIN).and_then(|v| v.to_str().ok()) {
        // No Origin header => non-browser client (e.g. native tooling). Allow.
        None => return true,
        Some(o) => o,
    };

    // Extract the authority (host[:port]) of the Origin URL: strip the scheme
    // and any trailing path.
    let origin_authority = origin
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(origin)
        .split('/')
        .next()
        .unwrap_or(origin);

    // Allow any loopback origin. Handle bracketed IPv6 literals (e.g.
    // `[::1]:8073`) before splitting off the port, so the colons inside the
    // address aren't mistaken for the port separator.
    let host_only = authority_host(origin_authority);
    if host_only.eq_ignore_ascii_case("localhost") || host_only == "127.0.0.1" || host_only == "::1"
    {
        return true;
    }

    // Allow same-origin: the Origin authority must match the request Host header.
    if let Some(host) = headers.get(HOST).and_then(|v| v.to_str().ok()) {
        if origin_authority.eq_ignore_ascii_case(host) {
            return true;
        }
    }

    false
}

/// Extract the host from an `host[:port]` authority, unwrapping a bracketed
/// IPv6 literal (`[::1]:8073` -> `::1`, `example.com:8073` -> `example.com`).
fn authority_host(authority: &str) -> &str {
    if let Some(rest) = authority.strip_prefix('[') {
        // IPv6 literal: host is everything up to the closing bracket.
        return rest.split(']').next().unwrap_or(rest);
    }
    authority.split(':').next().unwrap_or(authority)
}

async fn handle_socket(socket: WebSocket, state: SharedState, remote_addr: String) {
    let (per_client_tx, mut per_client_rx) = mpsc::channel::<ServerMessage>(64);
    let client_id = state.sessions.connect(remote_addr, per_client_tx).await;
    tracing::info!(client_id = %client_id, "WebSocket connected");

    let (mut sender, mut receiver) = socket.split();
    let mut waterfall_rx = state.waterfall_tx.subscribe();
    let mut decode_rx = state.decode_tx.subscribe();
    let mut radio_rx = state.radio_tx.subscribe();
    let mut qso_rx = state.qso_tx.subscribe();
    let mut log_rx = state.log_tx.subscribe();

    // Local auth flag — avoids per-message session lookups for the hot broadcast path
    let mut authenticated = !state.sessions.needs_viewer_auth();

    // Send Hello so the client knows whether to show the viewer password form.
    // When viewer auth is required and not yet satisfied, withhold station and
    // rig details — they must not be disclosed to unauthenticated clients. A
    // fresh Hello with the full values is re-sent after successful auth.
    let hello = build_hello(&state, authenticated);
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
                            // Re-send Hello with the now-authorised station/rig
                            // details that were withheld before authentication.
                            let hello = build_hello(&state, true);
                            let _ = send_msg(&mut sender, &hello).await;
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

            entry = log_rx.recv(), if authenticated => {
                match entry {
                    Ok(entry) => {
                        let msg = log_entry_msg(entry);
                        if send_msg(&mut sender, &msg).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(client_id = %client_id, "lagged by {n} log entries");
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

/// Build the `Hello` message. Station and rig details are only included when
/// `authenticated` is true; otherwise they are withheld to avoid disclosing
/// them to clients that have not yet passed viewer auth.
fn build_hello(state: &SharedState, authenticated: bool) -> ServerMessage {
    let cfg = state.config.read().unwrap();
    ServerMessage::Hello {
        needs_viewer_auth: state.sessions.needs_viewer_auth(),
        callsign: if authenticated {
            cfg.station.callsign.clone()
        } else {
            String::new()
        },
        grid: if authenticated {
            cfg.station.grid.clone()
        } else {
            String::new()
        },
        log_file: if authenticated {
            cfg.station.log_file.clone()
        } else {
            String::new()
        },
        rig_host: if authenticated {
            cfg.radio.rigctld_host.clone()
        } else {
            String::new()
        },
        rig_port: if authenticated {
            cfg.radio.rigctld_port
        } else {
            0
        },
        needs_setup: state.setup_mode.load(Ordering::Relaxed),
        os_type: state.os_type.to_string(),
        hamlib_available: cfg!(feature = "hamlib"),
    }
}

fn log_entry_msg(entry: LogEntryData) -> ServerMessage {
    ServerMessage::LogEntry {
        their_call: entry.their_call,
        their_grid: entry.their_grid,
        rst_sent: entry.rst_sent,
        rst_rcvd: entry.rst_rcvd,
        freq_hz: entry.freq_hz,
        band: entry.band,
        date: entry.date,
        time_on: entry.time_on,
    }
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
            freq: s.freq,
            mode: s.mode.clone(),
            ptt: s.ptt,
        };
        let _ = send_msg(sender, &msg).await;
    }

    // QSO state
    {
        let qso = state.qso.lock().await;
        let guard = state.tx_queue.lock().await;
        let msg = ServerMessage::QsoUpdate {
            state: serde_json::to_value(qso.clone()).unwrap_or_default(),
            next_tx: guard.as_ref().map(|r| r.message.clone()),
            tx_enabled: state.tx_enabled.load(Ordering::Relaxed),
            tx_queued: guard.is_some(),
        };
        let _ = send_msg(sender, &msg).await;
    }

    // Recent decodes (oldest first so client renders them in time order)
    {
        let cache = state.recent_decodes.lock().await;
        for period in cache.iter().rev() {
            let messages: Vec<DecodedMessageJson> = period
                .messages
                .iter()
                .map(|m| DecodedMessageJson {
                    snr: m.snr,
                    dt: m.dt,
                    freq: m.freq,
                    message: m.message.clone(),
                })
                .collect();
            let msg = ServerMessage::Decode {
                period: period.period,
                messages,
            };
            let _ = send_msg(sender, &msg).await;
        }
    }

    // Audio device list
    {
        let msg = ServerMessage::DeviceList {
            inputs: state.audio_input_devices.clone(),
            outputs: state.audio_output_devices.clone(),
        };
        let _ = send_msg(sender, &msg).await;
    }

    // Operator status (personalised for this client)
    {
        let op_id = state.sessions.current_operator().await;
        let count = state.sessions.client_count().await;
        let msg = ServerMessage::OperatorStatus {
            operator_client_id: op_id.map(|id| id.to_string()),
            you_are_operator: op_id == Some(client_id),
            client_count: count,
        };
        let _ = send_msg(sender, &msg).await;
    }
}

/// Returns `Ok(true)` when the client just became authenticated (so the caller
/// can trigger initial-state sync).
async fn handle_client_message(
    text: &str,
    state: &SharedState,
    sender: &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
    client_id: ClientId,
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
                let reply = ServerMessage::Error {
                    message: "Authentication required".into(),
                };
                let _ = send_msg(sender, &reply).await;
                return Ok(false);
            }
        }
    }

    // For operator-only commands, verify the lock before dispatching
    let is_operator_msg = |msg: &ClientMessage| {
        matches!(
            msg,
            ClientMessage::CallCq { .. }
                | ClientMessage::RespondTo { .. }
                | ClientMessage::QueueTx { .. }
                | ClientMessage::HaltTx {}
                | ClientMessage::EnableTx { .. }
                | ClientMessage::SetTxParity { .. }
                | ClientMessage::SetFrequency { .. }
                | ClientMessage::SetMode { .. }
                | ClientMessage::ResetQso {}
                | ClientMessage::ConfigUpdate { .. }
                | ClientMessage::TestRigctld {} // GetSerialPorts and CompleteSetup are intentionally NOT operator-only
        )
    };

    match serde_json::from_str::<ClientMessage>(text) {
        Ok(msg) => {
            if is_operator_msg(&msg) && !state.sessions.is_operator(client_id).await {
                let reply = ServerMessage::Error {
                    message: "Operator access required".into(),
                };
                let _ = send_msg(sender, &reply).await;
                return Ok(false);
            }

            match msg {
                ClientMessage::Ping {} => {
                    let reply = ServerMessage::Echo {
                        payload: serde_json::json!({ "pong": true }),
                    };
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
                        let reply = ServerMessage::Error {
                            message: "Wrong operator password".into(),
                        };
                        let _ = send_msg(sender, &reply).await;
                    }
                }

                ClientMessage::ReleaseOperator {} => {
                    state.sessions.release_operator(client_id).await;
                    state.sessions.broadcast_operator_status().await;
                }

                ClientMessage::SetFrequency { freq } => {
                    let _ = state
                        .radio_cmd_tx
                        .send(RadioCommand::SetFrequency(freq))
                        .await;
                }

                ClientMessage::SetMode { mode, passband } => {
                    let _ = state
                        .radio_cmd_tx
                        .send(RadioCommand::SetMode(mode, passband))
                        .await;
                }

                ClientMessage::EnableTx { enabled } => {
                    state.tx_enabled.store(enabled, Ordering::Relaxed);
                    tracing::info!("TX {}", if enabled { "enabled" } else { "disabled" });
                    broadcast_qso_update(state).await;
                }

                ClientMessage::SetTxParity { parity } => {
                    state
                        .desired_tx_parity
                        .store(parity != 0, Ordering::Relaxed);
                    tracing::info!("TX parity set to {}", parity);
                }

                ClientMessage::CallCq { freq } => {
                    let (my_call, my_grid) = {
                        let cfg = state.config.read().unwrap();
                        (cfg.station.callsign.clone(), cfg.station.grid.clone())
                    };
                    let msg_text = qso::cq_message(&my_call, &my_grid);

                    match encode_tx(&msg_text, freq, state.tx_sample_rate).await {
                        Ok(samples) => {
                            *state.qso.lock().await = QsoState::CallingCq {
                                my_call: my_call.clone(),
                                my_grid,
                                tx_freq: freq,
                            };
                            *state.qso_start.lock().unwrap() = Some(Utc::now());
                            *state.tx_queue.lock().await = Some(TxRequest {
                                samples,
                                message: msg_text.clone(),
                            });
                            state.tx_enabled.store(true, Ordering::Relaxed);
                            tracing::info!("CallCQ queued: {}", msg_text);
                            broadcast_qso_update(state).await;
                        }
                        Err(e) => {
                            tracing::error!("FT8 encode error for CQ: {e}");
                            let reply = ServerMessage::Error {
                                message: format!("encode error: {e}"),
                            };
                            let _ = send_msg(sender, &reply).await;
                        }
                    }
                }

                ClientMessage::RespondTo {
                    their_call,
                    their_freq: _,
                    tx_freq,
                } => {
                    let (my_call, my_grid) = {
                        let cfg = state.config.read().unwrap();
                        (cfg.station.callsign.clone(), cfg.station.grid.clone())
                    };
                    let msg_text = qso::grid_response(&their_call, &my_call, &my_grid);

                    match encode_tx(&msg_text, tx_freq, state.tx_sample_rate).await {
                        Ok(samples) => {
                            *state.qso.lock().await = QsoState::InQso {
                                their_call: their_call.clone(),
                                their_grid: None,
                                their_report: None,
                                my_report: None,
                                my_grid: Some(my_grid),
                                step: crate::engine::qso::QsoStep::SentGrid,
                                tx_freq,
                            };
                            *state.qso_start.lock().unwrap() = Some(Utc::now());
                            *state.tx_queue.lock().await = Some(TxRequest {
                                samples,
                                message: msg_text.clone(),
                            });
                            state.tx_enabled.store(true, Ordering::Relaxed);
                            tracing::info!("RespondTo {} queued: {}", their_call, msg_text);
                            broadcast_qso_update(state).await;
                        }
                        Err(e) => {
                            tracing::error!("FT8 encode error: {e}");
                            let reply = ServerMessage::Error {
                                message: format!("encode error: {e}"),
                            };
                            let _ = send_msg(sender, &reply).await;
                        }
                    }
                }

                ClientMessage::QueueTx { message, freq } => {
                    match encode_tx(&message, freq, state.tx_sample_rate).await {
                        Ok(samples) => {
                            *state.tx_queue.lock().await = Some(TxRequest {
                                samples,
                                message: message.clone(),
                            });
                            tracing::info!("Manual TX queued: {}", message);
                            broadcast_qso_update(state).await;
                        }
                        Err(e) => {
                            let reply = ServerMessage::Error {
                                message: format!("encode error: {e}"),
                            };
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
                    let _ = state.radio_cmd_tx.send(RadioCommand::SetPtt(false)).await;
                    tracing::info!("TX halted");
                    broadcast_qso_update(state).await;
                }

                ClientMessage::ResetQso {} => {
                    *state.qso.lock().await = QsoState::Idle;
                    *state.qso_start.lock().unwrap() = None;
                    state.tx_enabled.store(false, Ordering::Relaxed);
                    state.tx_queue.lock().await.take();
                    tracing::info!("QSO reset");
                    broadcast_qso_update(state).await;
                }

                ClientMessage::GetSerialPorts {} => {
                    let ports = list_serial_ports();
                    let reply = ServerMessage::SerialPortList { ports };
                    let _ = send_msg(sender, &reply).await;
                }

                ClientMessage::CompleteSetup {
                    callsign,
                    grid,
                    operator_password,
                    input_device,
                    output_device,
                    radio_backend,
                    rigctld_host,
                    rigctld_port,
                    rig_model,
                    serial_port,
                    baud_rate,
                } => {
                    let reply = handle_complete_setup(
                        state,
                        callsign,
                        grid,
                        operator_password,
                        input_device,
                        output_device,
                        radio_backend,
                        rigctld_host,
                        rigctld_port,
                        rig_model,
                        serial_port,
                        baud_rate,
                    )
                    .await;
                    let _ = send_msg(sender, &reply).await;
                }

                ClientMessage::ConfigUpdate { section, values } => {
                    let reply = handle_config_update(state, &section, &values).await;
                    let _ = send_msg(sender, &reply).await;
                }

                ClientMessage::TestRigctld {} => {
                    let (host, port) = {
                        let cfg = state.config.read().unwrap();
                        (cfg.radio.rigctld_host.clone(), cfg.radio.rigctld_port)
                    };
                    let reply = match crate::radio::hamlib::RigCtld::connect(&host, port).await {
                        Ok(_) => ServerMessage::RigctldTestResult {
                            success: true,
                            message: format!("Connected to {}:{}", host, port),
                        },
                        Err(e) => ServerMessage::RigctldTestResult {
                            success: false,
                            message: format!("Failed: {e}"),
                        },
                    };
                    let _ = send_msg(sender, &reply).await;
                }
            }

            Ok(false)
        }

        Err(e) => {
            tracing::warn!(client_id = %client_id, error = %e, "unknown message");
            let reply = ServerMessage::Error {
                message: format!("unknown message: {e}"),
            };
            let _ = send_msg(sender, &reply).await;
            Ok(false)
        }
    }
}

async fn handle_config_update(
    state: &SharedState,
    section: &str,
    values: &serde_json::Value,
) -> ServerMessage {
    let vals = match values.as_object() {
        Some(m) => m,
        None => {
            return ServerMessage::ConfigUpdateResult {
                success: false,
                message: Some("values must be an object".into()),
                requires_restart: false,
            }
        }
    };

    match section {
        "station" => {
            let callsign = vals
                .get("callsign")
                .and_then(|v| v.as_str())
                .map(|s| s.to_uppercase());
            let grid = vals
                .get("grid")
                .and_then(|v| v.as_str())
                .map(|s| s.to_uppercase());

            if let Some(ref c) = callsign {
                if !valid_callsign(c) {
                    return ServerMessage::ConfigUpdateResult {
                        success: false,
                        message: Some(format!("Invalid callsign: {c}")),
                        requires_restart: false,
                    };
                }
            }
            if let Some(ref g) = grid {
                if !valid_grid(g) {
                    return ServerMessage::ConfigUpdateResult {
                        success: false,
                        message: Some(format!("Invalid grid: {g}")),
                        requires_restart: false,
                    };
                }
            }

            {
                let mut cfg = state.config.write().unwrap();
                if let Some(c) = callsign {
                    cfg.station.callsign = c;
                }
                if let Some(g) = grid {
                    cfg.station.grid = g;
                }
                if let Err(e) = crate::config::save(&cfg) {
                    tracing::warn!("Config save failed: {e}");
                }
            }
            tracing::info!("Station config updated");
            ServerMessage::ConfigUpdateResult {
                success: true,
                message: None,
                requires_restart: false,
            }
        }

        "radio" => {
            {
                let mut cfg = state.config.write().unwrap();
                if let Some(h) = vals.get("rigctld_host").and_then(|v| v.as_str()) {
                    cfg.radio.rigctld_host = h.to_string();
                }
                if let Some(p) = vals.get("rigctld_port").and_then(|v| v.as_u64()) {
                    cfg.radio.rigctld_port = p as u16;
                }
                if let Err(e) = crate::config::save(&cfg) {
                    tracing::warn!("Config save failed: {e}");
                }
            }
            tracing::info!("Radio config updated");
            ServerMessage::ConfigUpdateResult {
                success: true,
                message: Some("Radio config saved; restart to apply.".into()),
                requires_restart: true,
            }
        }

        "audio" => {
            {
                let mut cfg = state.config.write().unwrap();
                if let Some(d) = vals.get("input_device").and_then(|v| v.as_str()) {
                    cfg.audio.input_device = if d.is_empty() {
                        None
                    } else {
                        Some(d.to_string())
                    };
                }
                if let Some(d) = vals.get("output_device").and_then(|v| v.as_str()) {
                    cfg.audio.output_device = if d.is_empty() {
                        None
                    } else {
                        Some(d.to_string())
                    };
                }
                if let Err(e) = crate::config::save(&cfg) {
                    tracing::warn!("Config save failed: {e}");
                }
            }
            tracing::info!("Audio config updated");
            ServerMessage::ConfigUpdateResult {
                success: true,
                message: Some("Audio config saved; restart to apply.".into()),
                requires_restart: true,
            }
        }

        other => ServerMessage::ConfigUpdateResult {
            success: false,
            message: Some(format!("Unknown config section: {other}")),
            requires_restart: false,
        },
    }
}

fn list_serial_ports() -> Vec<String> {
    match serialport::available_ports() {
        Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
        Err(e) => {
            tracing::warn!("Failed to enumerate serial ports: {e}");
            vec![]
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_complete_setup(
    state: &SharedState,
    callsign: String,
    grid: String,
    operator_password: String,
    input_device: Option<String>,
    output_device: Option<String>,
    radio_backend: String,
    rigctld_host: String,
    rigctld_port: u16,
    rig_model: Option<i32>,
    serial_port: Option<String>,
    baud_rate: Option<u32>,
) -> ServerMessage {
    // Only permit the setup wizard during genuine first-run setup mode. Without
    // this gate any auto-authenticated viewer could overwrite the operator
    // password and seize operator control. Once setup has completed the server
    // leaves setup mode and this path is rejected.
    if !state.setup_mode.load(Ordering::Relaxed) {
        tracing::warn!("Rejected CompleteSetup: server is not in setup mode");
        return ServerMessage::ConfigUpdateResult {
            success: false,
            message: Some("Setup has already been completed".into()),
            requires_restart: false,
        };
    }

    let callsign = callsign.to_uppercase();
    let grid = grid.to_uppercase();

    if !valid_callsign(&callsign) {
        return ServerMessage::ConfigUpdateResult {
            success: false,
            message: Some(format!("Invalid callsign: {callsign}")),
            requires_restart: false,
        };
    }
    if !valid_grid(&grid) {
        return ServerMessage::ConfigUpdateResult {
            success: false,
            message: Some(format!("Invalid grid: {grid}")),
            requires_restart: false,
        };
    }
    if operator_password.is_empty() {
        return ServerMessage::ConfigUpdateResult {
            success: false,
            message: Some("Operator password cannot be empty".into()),
            requires_restart: false,
        };
    }

    {
        let mut cfg = state.config.write().unwrap();
        cfg.station.callsign = callsign;
        cfg.station.grid = grid;
        cfg.network.operator_password = operator_password.clone();
        cfg.audio.input_device = input_device.filter(|s| !s.is_empty());
        cfg.audio.output_device = output_device.filter(|s| !s.is_empty());
        cfg.radio.backend = radio_backend;
        cfg.radio.rigctld_host = rigctld_host;
        cfg.radio.rigctld_port = rigctld_port;
        cfg.radio.rig_model = rig_model;
        cfg.radio.serial_port = serial_port.filter(|s| !s.is_empty());
        cfg.radio.baud_rate = baud_rate;
        if let Err(e) = crate::config::save(&cfg) {
            tracing::warn!("Config save failed during setup: {e}");
            return ServerMessage::ConfigUpdateResult {
                success: false,
                message: Some(format!("Failed to save config: {e}")),
                requires_restart: false,
            };
        }
    }

    state
        .sessions
        .update_operator_password(operator_password)
        .await;
    state.setup_mode.store(false, Ordering::Relaxed);
    tracing::info!("Setup wizard completed; config saved");

    ServerMessage::ConfigUpdateResult {
        success: true,
        message: Some(
            "Setup complete — restart the server to activate audio and radio settings.".into(),
        ),
        requires_restart: true,
    }
}

fn valid_callsign(call: &str) -> bool {
    let len = call.len();
    (3..=13).contains(&len)
        && call
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-')
}

fn valid_grid(grid: &str) -> bool {
    let b = grid.as_bytes();
    b.len() >= 4
        && b.len() <= 6
        && b[0].is_ascii_alphabetic()
        && b[1].is_ascii_alphabetic()
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
}

/// Encode FT8 audio in a blocking thread pool.
async fn encode_tx(message: &str, freq: f32, sample_rate: u32) -> anyhow::Result<Vec<f32>> {
    let message = message.to_string();
    tokio::task::spawn_blocking(move || crate::dsp::ft8::encode(&message, freq, sample_rate))
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking: {e}"))?
}

/// Push a fresh QSO update to all subscribers via the broadcast channel.
async fn broadcast_qso_update(state: &SharedState) {
    let qso = state.qso.lock().await;
    let guard = state.tx_queue.lock().await;
    let update = QsoUpdate {
        state: qso.clone(),
        next_tx: guard.as_ref().map(|r| r.message.clone()),
        tx_enabled: state.tx_enabled.load(Ordering::Relaxed),
        tx_queued: guard.is_some(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::{HOST, ORIGIN};

    fn headers(origin: Option<&str>, host: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(o) = origin {
            h.insert(ORIGIN, o.parse().unwrap());
        }
        if let Some(host) = host {
            h.insert(HOST, host.parse().unwrap());
        }
        h
    }

    #[test]
    fn authority_host_handles_ipv6_and_ports() {
        assert_eq!(authority_host("[::1]:8073"), "::1");
        assert_eq!(authority_host("[::1]"), "::1");
        assert_eq!(authority_host("example.com:8073"), "example.com");
        assert_eq!(authority_host("127.0.0.1:8073"), "127.0.0.1");
        assert_eq!(authority_host("localhost"), "localhost");
    }

    #[test]
    fn origin_allowed_permits_loopback_including_ipv6() {
        assert!(origin_allowed(&headers(
            Some("http://localhost:8073"),
            None
        )));
        assert!(origin_allowed(&headers(
            Some("http://127.0.0.1:8073"),
            None
        )));
        // The IPv6 loopback must not be rejected by naive `:`-splitting.
        assert!(origin_allowed(&headers(Some("http://[::1]:8073"), None)));
    }

    #[test]
    fn origin_allowed_permits_same_origin_and_absent() {
        assert!(origin_allowed(&headers(None, Some("radio.example:8073"))));
        assert!(origin_allowed(&headers(
            Some("https://radio.example:8073"),
            Some("radio.example:8073"),
        )));
    }

    #[test]
    fn origin_allowed_rejects_cross_origin() {
        assert!(!origin_allowed(&headers(
            Some("https://evil.example"),
            Some("radio.example:8073"),
        )));
        // A path in the Origin must not smuggle the host past the check.
        assert!(!origin_allowed(&headers(
            Some("https://evil.example/radio.example"),
            Some("radio.example"),
        )));
    }
}
