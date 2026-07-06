use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::IntoResponse,
    routing::{any, get},
    Router,
};
use rust_embed::Embed;
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::state::SharedState;
use super::ws_handler::ws_handler;

/// Client SPA assets embedded at compile time from `client/dist/`.
#[derive(Embed)]
#[folder = "../client/dist/"]
struct ClientAssets;

pub fn build_router(state: SharedState) -> Router {
    let static_files = state.config.read().unwrap().network.static_files.clone();

    let router = Router::new()
        .route("/ws", any(ws_handler))
        .route("/api/log", get(download_log));

    // If static_files is configured and the directory exists, serve from disk
    // (useful for development). Otherwise, serve the embedded client assets.
    let router = if let Some(ref dir) = static_files {
        if std::path::Path::new(dir).is_dir() {
            tracing::info!("Serving static files from filesystem: {dir}");
            router.fallback_service(ServeDir::new(dir))
        } else {
            tracing::info!("Serving embedded client assets");
            router.fallback(serve_embedded)
        }
    } else {
        tracing::info!("Serving embedded client assets");
        router.fallback(serve_embedded)
    };

    // The SPA is served from the same origin as the API/WebSocket, so no
    // cross-origin access is required. Use a default (non-permissive) CORS layer
    // so we do not echo arbitrary `Origin`s or enable credentialed cross-site
    // requests. Cross-site WebSocket upgrades are separately blocked by the
    // Origin check in `ws_handler`.
    router
        .with_state(state)
        .layer(CorsLayer::new())
}

/// Serve files from the embedded client assets, with SPA fallback to index.html.
async fn serve_embedded(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = <ClientAssets as Embed>::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        (
            [(header::CONTENT_TYPE, mime.as_ref().to_string())],
            file.data.to_vec(),
        )
            .into_response()
    } else if let Some(index) = <ClientAssets as Embed>::get("index.html") {
        // SPA fallback: serve index.html for client-side routes
        (
            [(header::CONTENT_TYPE, "text/html".to_string())],
            index.data.to_vec(),
        )
            .into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn download_log(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    // Auth is otherwise WS-only, so gate this HTTP route with the same viewer
    // password. Accept the credential either as a `?token=` query parameter or an
    // `Authorization: Bearer <token>` header. When no viewer password is set,
    // viewing is open and the log stays downloadable without a token.
    let token = params.get("token").map(|s| s.as_str()).or_else(|| {
        headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.strip_prefix("Bearer ").unwrap_or(v).trim())
    });
    if !state.sessions.check_viewer_credential(token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

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
