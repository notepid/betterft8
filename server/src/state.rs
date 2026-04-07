use std::sync::Arc;
use crate::config::Config;

pub struct AppState {
    pub config: Config,
}

pub type SharedState = Arc<AppState>;
