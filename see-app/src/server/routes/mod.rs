mod agents;
mod config;
mod dashboard;
mod health;
mod logs;
mod skills;
mod teams;
mod tools;

use axum::Router;

use super::AppState;

/// Combine all API sub-routers under `/api`.
pub fn api_router(state: AppState) -> Router {
    Router::new()
        .merge(health::router())
        .merge(agents::router(state.clone()))
        .merge(teams::router(state.clone()))
        .merge(tools::router(state.clone()))
        .merge(skills::router(state.clone()))
        .merge(config::router(state.clone()))
        .merge(dashboard::router(state.clone()))
        .merge(logs::router(state))
}
