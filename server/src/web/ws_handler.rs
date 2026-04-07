use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::state::SharedState;
use super::messages::{ClientMessage, ServerMessage};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, _state: SharedState) {
    let conn_id = Uuid::new_v4();
    tracing::info!(conn_id = %conn_id, "WebSocket connected");

    let (mut sender, mut receiver) = socket.split();

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
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
            Message::Close(_) => break,
            _ => {}
        }
    }

    tracing::info!(conn_id = %conn_id, "WebSocket disconnected");
}
