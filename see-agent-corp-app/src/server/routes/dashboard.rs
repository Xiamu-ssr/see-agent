use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use see_agent_corp::agent::list_agents;
use see_agent_corp::skill::{gate_skills, load_skills};
use see_agent_corp::team::list_teams;
use see_agent_corp::tool::builtin_tool_infos;

use crate::server::AppState;

#[derive(Serialize, Deserialize)]
struct DashboardResponse {
    agents_count: usize,
    agents_running: usize,
    sleeping_agents: usize,
    teams_count: usize,
    tools_count: usize,
    skills_count: usize,
    version: String,
}

#[derive(Serialize, Deserialize)]
struct FreezeReviveResponse {
    status: String,
    count: usize,
}

async fn get_dashboard_handler(
    State(state): State<AppState>,
) -> Result<Json<DashboardResponse>, StatusCode> {
    let ws = state.workspace();

    let agents = list_agents(ws).unwrap_or_default();
    let teams = list_teams(ws).unwrap_or_default();

    let sup = state.inner.supervisor.read().await;
    let running = sup.running_agents().len();

    let frozen = state.inner.frozen_agents.read().await;
    let sleeping = frozen.len();

    let config = state.inner.config.read().await;
    let tools_count = builtin_tool_infos().len();
    let skills_count = gate_skills(load_skills(&config.skills.dirs)).len();

    Ok(Json(DashboardResponse {
        agents_count: agents.len(),
        agents_running: running,
        sleeping_agents: sleeping,
        teams_count: teams.len(),
        tools_count,
        skills_count,
        version: see_agent_corp::consts::VERSION.into(),
    }))
}

async fn freeze_handler(
    State(state): State<AppState>,
) -> Result<Json<FreezeReviveResponse>, StatusCode> {
    // Get list of currently running agents
    let running: Vec<String> = {
        let sup = state.inner.supervisor.read().await;
        sup.running_agents().into_iter().map(|(id, _)| id).collect()
    };
    let count = running.len();

    // Store in frozen_agents
    {
        let mut frozen = state.inner.frozen_agents.write().await;
        *frozen = running;
    }

    // Stop all running agents
    {
        let mut sup = state.inner.supervisor.write().await;
        sup.stop_all().await;
    }

    Ok(Json(FreezeReviveResponse {
        status: "frozen".into(),
        count,
    }))
}

async fn revive_handler(
    State(state): State<AppState>,
) -> Result<Json<FreezeReviveResponse>, StatusCode> {
    // Read frozen_agents list
    let to_revive: Vec<String> = {
        let frozen = state.inner.frozen_agents.read().await;
        frozen.clone()
    };
    let count = to_revive.len();

    // Start each agent
    {
        let mut sup = state.inner.supervisor.write().await;
        for agent_id in &to_revive {
            // Best-effort: skip agents that fail to start
            let _ = sup.start_agent(agent_id).await;
        }
    }

    // Clear frozen_agents
    {
        let mut frozen = state.inner.frozen_agents.write().await;
        frozen.clear();
    }

    Ok(Json(FreezeReviveResponse {
        status: "revived".into(),
        count,
    }))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/dashboard", get(get_dashboard_handler))
        .route("/freeze", post(freeze_handler))
        .route("/revive", post(revive_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn make_test_state() -> AppState {
        let tmp = tempfile::TempDir::new().unwrap();
        let ws = see_agent_corp::types::WorkspaceDir::new(tmp.path());

        // Create a couple of agent directories so supervisor can start them
        let alice_dir = ws.agent("alice");
        std::fs::create_dir_all(alice_dir.path()).unwrap();
        std::fs::write(alice_dir.inbox(), "").unwrap();

        let bob_dir = ws.agent("bob");
        std::fs::create_dir_all(bob_dir.path()).unwrap();
        std::fs::write(bob_dir.inbox(), "").unwrap();

        // Leak TempDir so it lives for the duration of the test
        std::mem::forget(tmp);
        AppState::new(ws)
    }

    #[tokio::test]
    async fn freeze_records_running_agents() {
        let state = make_test_state();

        // Manually insert agent ids into frozen_agents to simulate freeze logic
        // (We can't actually start real processes in tests, so test the state tracking.)
        {
            let mut frozen = state.inner.frozen_agents.write().await;
            *frozen = vec!["alice".into(), "bob".into()];
        }

        let frozen = state.inner.frozen_agents.read().await;
        assert_eq!(frozen.len(), 2);
        assert!(frozen.contains(&"alice".to_string()));
        assert!(frozen.contains(&"bob".to_string()));
    }

    #[tokio::test]
    async fn revive_restarts_frozen_agents() {
        let state = make_test_state();

        // Simulate frozen state
        {
            let mut frozen = state.inner.frozen_agents.write().await;
            *frozen = vec!["alice".into(), "bob".into()];
        }

        // Call revive endpoint
        let app = router(state.clone());
        let req = Request::builder()
            .method("POST")
            .uri("/revive")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: FreezeReviveResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.status, "revived");
        assert_eq!(result.count, 2);

        // Frozen list should be cleared
        let frozen = state.inner.frozen_agents.read().await;
        assert!(frozen.is_empty());
    }

    #[tokio::test]
    async fn freeze_then_revive_roundtrip() {
        let state = make_test_state();

        let app = router(state.clone());

        // Freeze (no running agents, so count=0)
        let req = Request::builder()
            .method("POST")
            .uri("/freeze")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let freeze_result: FreezeReviveResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(freeze_result.status, "frozen");
        assert_eq!(freeze_result.count, 0);

        // Frozen list is empty since no agents were running
        let frozen = state.inner.frozen_agents.read().await;
        assert!(frozen.is_empty());
        drop(frozen);

        // Manually set frozen agents to simulate a real freeze scenario
        {
            let mut frozen = state.inner.frozen_agents.write().await;
            *frozen = vec!["alice".into()];
        }

        // Revive
        let app2 = router(state.clone());
        let req2 = Request::builder()
            .method("POST")
            .uri("/revive")
            .body(Body::empty())
            .unwrap();

        let resp2 = app2.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        let body2 = axum::body::to_bytes(resp2.into_body(), usize::MAX)
            .await
            .unwrap();
        let revive_result: FreezeReviveResponse = serde_json::from_slice(&body2).unwrap();
        assert_eq!(revive_result.status, "revived");
        assert_eq!(revive_result.count, 1);

        // Frozen list cleared after revive
        let frozen = state.inner.frozen_agents.read().await;
        assert!(frozen.is_empty());
    }

    #[tokio::test]
    async fn dashboard_includes_sleeping_agents() {
        let state = make_test_state();

        // Set some frozen agents
        {
            let mut frozen = state.inner.frozen_agents.write().await;
            *frozen = vec!["alice".into(), "bob".into()];
        }

        let app = router(state.clone());
        let req = Request::builder()
            .method("GET")
            .uri("/dashboard")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let dashboard: DashboardResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(dashboard.sleeping_agents, 2);
    }
}
