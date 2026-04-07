use axum::{routing::any, Router};
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::state::SharedState;
use super::ws_handler::ws_handler;

pub fn build_router(state: SharedState) -> Router {
    let static_files = state.config.network.static_files.clone();

    Router::new()
        .route("/ws", any(ws_handler))
        .fallback_service(ServeDir::new(&static_files))
        .with_state(state)
        .layer(CorsLayer::permissive())
}
