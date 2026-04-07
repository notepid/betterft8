use serde::{Deserialize, Serialize};

/// Messages sent from Server → Client
#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Echo { payload: serde_json::Value },
    Error { message: String },
    Waterfall {
        timestamp: f64,
        freq_min:  u32,
        freq_max:  u32,
        data:      String, // base64
    },
    Decode {
        period:   u64,
        messages: Vec<DecodedMessageJson>,
    },
}

#[derive(Serialize, Clone)]
pub struct DecodedMessageJson {
    pub snr:     i32,
    pub dt:      f32,
    pub freq:    f32,
    pub message: String,
}

/// Messages sent from Client → Server
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Ping {},
}
