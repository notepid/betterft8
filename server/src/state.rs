use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use chrono::{DateTime, Utc};
use tokio::sync::{broadcast, Mutex};

use crate::audio::playback::PlaybackHandle;
use crate::config::Config;
use crate::dsp::ft8::DecodedMessage;
use crate::engine::qso::QsoState;
use crate::radio::hamlib::RigCtld;
use crate::web::session::SessionManager;

// ---- Broadcast payloads -----------------------------------------------------

#[derive(Clone)]
pub struct WaterfallLine {
    pub timestamp: f64,
    pub data_b64:  String,
    pub freq_min:  u32,
    pub freq_max:  u32,
}

#[derive(Clone)]
pub struct DecodeResult {
    pub period:   u64,
    pub messages: Vec<DecodedMessage>,
}

#[derive(Clone, Default)]
pub struct RadioStatus {
    pub connected: bool,
    pub freq:      u64,
    pub mode:      String,
    pub ptt:       bool,
}

/// A pending FT8 transmission: pre-encoded audio + display text.
pub struct TxRequest {
    /// Pre-encoded audio samples at `AppState::tx_sample_rate` Hz.
    pub samples: Vec<f32>,
    /// Human-readable message text (for display and QSO state tracking).
    pub message: String,
}

/// QSO state update broadcast to all WebSocket clients.
#[derive(Clone)]
pub struct QsoUpdate {
    pub state:      QsoState,
    pub next_tx:    Option<String>,
    pub tx_enabled: bool,
    pub tx_queued:  bool,
}

/// ADIF log entry broadcast to all authenticated clients.
#[derive(Clone)]
pub struct LogEntryData {
    pub their_call: String,
    pub their_grid: Option<String>,
    pub rst_sent:   String,
    pub rst_rcvd:   String,
    pub freq_hz:    u64,
    pub band:       String,
    pub date:       String,
    pub time_on:    String,
}

// ---- Shared application state -----------------------------------------------

pub struct AppState {
    /// In-memory configuration behind a RwLock so Settings panel can mutate it.
    pub config: std::sync::RwLock<Config>,

    // ---- Session management -------------------------------------------------
    pub sessions: SessionManager,

    // ---- Broadcast channels -------------------------------------------------
    pub waterfall_tx: broadcast::Sender<WaterfallLine>,
    pub decode_tx:    broadcast::Sender<DecodeResult>,
    pub radio_tx:     broadcast::Sender<RadioStatus>,
    pub qso_tx:       broadcast::Sender<QsoUpdate>,
    pub log_tx:       broadcast::Sender<LogEntryData>,

    // ---- Cached state for initial sync to new clients -----------------------
    /// Last 5 decode periods; newest at front.
    pub recent_decodes: Mutex<VecDeque<DecodeResult>>,
    /// Last known radio status.
    pub last_radio_status: Mutex<RadioStatus>,

    // ---- Radio control ------------------------------------------------------
    pub rig: Mutex<Option<RigCtld>>,

    // ---- TX / QSO -----------------------------------------------------------
    /// Pre-encoded audio waiting to fire on the next TX period.
    pub tx_queue: Mutex<Option<TxRequest>>,
    /// Master TX enable flag; if false the timing engine never keys PTT.
    pub tx_enabled: AtomicBool,
    /// false = even periods (0,30 s), true = odd periods (15,45 s).
    pub desired_tx_parity: AtomicBool,
    /// Current QSO state machine.
    pub qso: Mutex<QsoState>,
    /// Timestamp when the current QSO started (used for ADIF TIME_ON).
    pub qso_start: std::sync::Mutex<Option<DateTime<Utc>>>,
    /// Audio output handle (None if no output device is available).
    pub playback: Option<PlaybackHandle>,
    /// Sample rate of the audio output (or 12000 if no playback device).
    pub tx_sample_rate: u32,

    // ---- Audio device enumeration -------------------------------------------
    pub audio_input_devices:  Vec<String>,
    pub audio_output_devices: Vec<String>,
}

pub type SharedState = Arc<AppState>;
