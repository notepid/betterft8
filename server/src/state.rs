use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use crate::config::Config;
use crate::dsp::ft8::DecodedMessage;
use crate::radio::hamlib::RigCtld;

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

pub struct AppState {
    pub config:       Config,
    pub waterfall_tx: broadcast::Sender<WaterfallLine>,
    pub decode_tx:    broadcast::Sender<DecodeResult>,
    pub radio_tx:     broadcast::Sender<RadioStatus>,
    pub rig:          Mutex<Option<RigCtld>>,
}

pub type SharedState = Arc<AppState>;
