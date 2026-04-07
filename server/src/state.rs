use std::sync::Arc;
use tokio::sync::broadcast;
use crate::config::Config;
use crate::dsp::ft8::DecodedMessage;

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

pub struct AppState {
    pub config:       Config,
    pub waterfall_tx: broadcast::Sender<WaterfallLine>,
    pub decode_tx:    broadcast::Sender<DecodeResult>,
}

pub type SharedState = Arc<AppState>;
