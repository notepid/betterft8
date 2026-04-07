use serde::{Deserialize, Serialize};

/// Messages sent from Server → Client
#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Echo { payload: serde_json::Value },
    Error { message: String },
}

/// Messages sent from Client → Server
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Ping {},
}
