use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::state::SharedState;
use super::messages::{ClientMessage, ServerMessage};

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

    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(ClientMessage::Ping {}) => {
                                tracing::debug!(conn_id = %conn_id, "ping received");
                                let reply = ServerMessage::Echo {
                                    payload: serde_json::json!({ "pong": true }),
                                };
                                let json = serde_json::to_string(&reply).unwrap();
                                if sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(conn_id = %conn_id, error = %e, "unknown message");
                                let reply = ServerMessage::Error {
                                    message: format!("unknown message: {e}"),
                                };
                                let json = serde_json::to_string(&reply).unwrap();
                                let _ = sender.send(Message::Text(json.into())).await;
                            }
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
                            freq_min: line.freq_min,
                            freq_max: line.freq_max,
                            data: line.data_b64,
                        };
                        let json = serde_json::to_string(&msg).unwrap();
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(conn_id = %conn_id, "client lagged by {n} waterfall lines");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    tracing::info!(conn_id = %conn_id, "WebSocket disconnected");
}
