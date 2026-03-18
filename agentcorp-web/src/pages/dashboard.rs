use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct DashboardData {
    agents_count: usize,
    agents_running: usize,
    teams_count: usize,
    tools_count: usize,
    skills_count: usize,
    version: String,
}

#[component]
pub fn Dashboard() -> impl IntoView {
    let data = LocalResource::new(|| async {
        api::get::<DashboardData>("/dashboard").await.ok()
    });

    view! {
        <div class="page">
            <h2>"Dashboard"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || data.get().map(|d| {
                    match &*d {
                        Some(d) => view! {
                            <div class="stats">
                                <div class="stat-card">
                                    <span class="stat-label">"Version"</span>
                                    <span class="stat-value">{d.version.clone()}</span>
                                </div>
                                <div class="stat-card">
                                    <span class="stat-label">"Total Agents"</span>
                                    <span class="stat-value">{d.agents_count}</span>
                                </div>
                                <div class="stat-card">
                                    <span class="stat-label">"Running"</span>
                                    <span class="stat-value stat-running">{d.agents_running}</span>
                                </div>
                                <div class="stat-card">
                                    <span class="stat-label">"Teams"</span>
                                    <span class="stat-value">{d.teams_count}</span>
                                </div>
                                <div class="stat-card">
                                    <span class="stat-label">"Tools"</span>
                                    <span class="stat-value">{d.tools_count}</span>
                                </div>
                                <div class="stat-card">
                                    <span class="stat-label">"Skills"</span>
                                    <span class="stat-value">{d.skills_count}</span>
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
