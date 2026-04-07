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
    RadioStatus {
        connected: bool,
        freq:      u64,
        mode:      String,
        ptt:       bool,
    },
    /// Full QSO state update (sent after every decode cycle and on TX commands).
    QsoUpdate {
        /// Serialised `QsoState` — tagged by `"state"` field.
        state:      serde_json::Value,
        /// The message text currently queued for the next TX period.
        next_tx:    Option<String>,
        tx_enabled: bool,
        tx_queued:  bool,
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
    SetFrequency { freq: u64 },
    SetMode { mode: String, passband: i32 },

    /// Begin calling CQ on `freq` Hz (audio frequency within passband).
    CallCq { freq: f32 },
    /// Respond to a decoded CQ from `their_call` at `their_freq`.
    /// `tx_freq` is our chosen audio TX frequency.
    RespondTo { their_call: String, their_freq: f32, tx_freq: f32 },
    /// Manually queue a custom FT8 message.
    QueueTx { message: String, freq: f32 },
    /// Emergency stop: drop TX queue and deassert PTT.
    HaltTx {},
    /// Enable or disable automatic TX.
    EnableTx { enabled: bool },
    /// Select TX period parity: 0 = even, 1 = odd.
    SetTxParity { parity: u8 },
    /// Reset QSO state to Idle.
    ResetQso {},
}
