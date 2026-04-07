use std::sync::Arc;
use tokio::sync::broadcast;
use crate::config::Config;

#[derive(Clone)]
pub struct WaterfallLine {
    pub timestamp: f64,
    pub data_b64: String,
    pub freq_min: u32,
    pub freq_max: u32,
}

pub struct AppState {
    pub config: Config,
    pub waterfall_tx: broadcast::Sender<WaterfallLine>,
}

pub type SharedState = Arc<AppState>;
