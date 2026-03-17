mod routes;
mod state;

pub use state::AppState;

use std::net::SocketAddr;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use see::consts::DEFAULT_SERVER_PORT;

/// Build the axum Router with all API routes.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = routes::api_router(state);

    Router::new().nest("/api", api).layer(cors)
}

/// Start the HTTP server.
pub async fn serve(state: AppState, port: Option<u16>) {
    let port = port.unwrap_or(DEFAULT_SERVER_PORT);
    let router = build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("see-agent server listening on http://0.0.0.0:{port}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, router)
        .await
        .expect("server error");
}
