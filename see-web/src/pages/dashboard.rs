use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct HealthResponse {
    status: String,
    agents: usize,
    teams: usize,
}

#[component]
pub fn Dashboard() -> impl IntoView {
    let health = LocalResource::new(|| async {
        api::get::<HealthResponse>("/health").await.ok()
    });

    view! {
        <div class="page">
            <h2>"Dashboard"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || health.get().map(|h| {
                    match &*h {
                        Some(h) => view! {
                            <div class="stats">
                                <div class="stat-card">
                                    <span class="stat-label">"Status"</span>
                                    <span class="stat-value">{h.status.clone()}</span>
                                </div>
                                <div class="stat-card">
                                    <span class="stat-label">"Agents"</span>
                                    <span class="stat-value">{h.agents}</span>
                                </div>
                                <div class="stat-card">
                                    <span class="stat-label">"Teams"</span>
                                    <span class="stat-value">{h.teams}</span>
                                </div>
                            </div>
                        }.into_any(),
                        None => view! {
                            <p class="error">"Could not connect to server"</p>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
