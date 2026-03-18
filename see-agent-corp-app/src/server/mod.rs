mod frontend;
mod routes;
mod state;

pub use state::AppState;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::routing::get;
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use see_agent_corp::consts::DEFAULT_SERVER_PORT;

use crate::cli::daemon;

/// Build the axum Router with all API routes.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = routes::api_router(state);

    Router::new()
        .nest("/api", api)
        .route("/", get(frontend::serve_index))
        .route("/{*path}", get(frontend::serve_frontend))
        .layer(cors)
}

/// Start the HTTP server.
///
/// If `pid_file` is provided, the server writes its PID on startup and removes it on shutdown.
pub async fn serve(state: AppState, port: Option<u16>, pid_file: Option<PathBuf>) {
    let port = port.unwrap_or(DEFAULT_SERVER_PORT);
    let router = build_router(state.clone());
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    // Bind first — fail fast if port is taken
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    // Only write PID after successful bind
    if let Some(ref pf) = pid_file {
        daemon::write_pid(pf);
    }

    info!("see-agent-corp server listening on http://0.0.0.0:{port}");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");

    // Stop all running worker processes before exit
    info!("stopping all worker processes");
    state.inner.supervisor.write().await.stop_all().await;

    if let Some(ref pf) = pid_file {
        daemon::remove_pid(pf);
    }

    info!("server shut down gracefully");
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to register SIGTERM handler");

        tokio::select! {
            _ = ctrl_c => info!("received SIGINT, shutting down"),
            _ = sigterm.recv() => info!("received SIGTERM, shutting down"),
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.expect("failed to listen for ctrl-c");
        info!("received SIGINT, shutting down");
    }
}
