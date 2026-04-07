use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{any, get},
    Router,
};
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::state::SharedState;
use super::ws_handler::ws_handler;

pub fn build_router(state: SharedState) -> Router {
    let static_files = state.config.read().unwrap().network.static_files.clone();

    Router::new()
        .route("/ws", any(ws_handler))
        .route("/api/log", get(download_log))
        .fallback_service(ServeDir::new(&static_files))
        .with_state(state)
        .layer(CorsLayer::permissive())
}

async fn download_log(State(state): State<SharedState>) -> impl IntoResponse {
    let path = state.config.read().unwrap().station.log_file.clone();
    match tokio::fs::read(&path).await {
        Ok(content) => (
            [
                (header::CONTENT_DISPOSITION, "attachment; filename=\"ft8.adi\""),
                (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            ],
            content,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
