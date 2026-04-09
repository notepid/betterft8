use serde::{Deserialize, Serialize};

/// Messages sent from Server → Client
#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Echo { payload: serde_json::Value },
    Error { message: String },
    /// First message sent to every client on connect.
    Hello {
        needs_viewer_auth: bool,
        callsign:          String,
        grid:              String,
        log_file:          String,
        rig_host:          String,
        rig_port:          u16,
        needs_setup:       bool,
        os_type:           String,
        hamlib_available:  bool,
    },
    /// Response to `Auth` (viewer) or indicates auth state.
    AuthResult { success: bool },
    /// Broadcast whenever operator lock changes; personalised per-client.
    OperatorStatus {
        operator_client_id: Option<String>,
        you_are_operator: bool,
        client_count: usize,
    },
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
    /// Broadcast when a QSO is successfully logged to ADIF.
    LogEntry {
        their_call: String,
        their_grid: Option<String>,
        rst_sent:   String,
        rst_rcvd:   String,
        freq_hz:    u64,
        band:       String,
        date:       String,
        time_on:    String,
    },
    /// Sent on connect and on demand: available audio devices.
    DeviceList {
        inputs:  Vec<String>,
        outputs: Vec<String>,
    },
    /// Response to a `ConfigUpdate` client message.
    ConfigUpdateResult {
        success:          bool,
        message:          Option<String>,
        requires_restart: bool,
    },
    /// Response to a `TestRigctld` client message.
    RigctldTestResult {
        success: bool,
        message: String,
    },
    /// Response to a `GetSerialPorts` client message.
    SerialPortList { ports: Vec<String> },
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
    /// Viewer password authentication (only required if `needs_viewer_auth` was true).
    Auth { password: String },
    /// Claim the operator lock.
    ClaimOperator { password: String },
    /// Release the operator lock.
    ReleaseOperator {},

    SetFrequency { freq: u64 },
    SetMode { mode: String, passband: i32 },

    /// Begin calling CQ on `freq` Hz (audio frequency within passband).
    CallCq { freq: f32 },
    /// Respond to a decoded CQ from `their_call` at `their_freq`.
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

    /// Update a config section. Operator-only.
    /// `section`: "station" | "audio" | "radio" | "network"
    /// `values`: map of field name → new value (as JSON strings/numbers)
    ConfigUpdate { section: String, values: serde_json::Value },

    /// Test the configured rigctld connection. Operator-only.
    TestRigctld {},
    /// Request the list of available serial ports (for Hamlib direct setup).
    GetSerialPorts {},
    /// Complete the setup wizard: saves all config sections at once.
    /// Does not require operator lock; works on first run.
    CompleteSetup {
        callsign:          String,
        grid:              String,
        operator_password: String,
        input_device:      Option<String>,
        output_device:     Option<String>,
        radio_backend:     String,
        rigctld_host:      String,
        rigctld_port:      u16,
        rig_model:         Option<i32>,
        serial_port:       Option<String>,
        baud_rate:         Option<u32>,
    },
}
