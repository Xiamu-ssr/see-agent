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
        <div>
            <h2 class="text-xl font-bold mb-4">"Dashboard"</h2>
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || data.get().map(|d| {
                    match &*d {
                        Some(d) => {
                            let version = d.version.clone();
                            let agents_count = d.agents_count.to_string();
                            let agents_running = d.agents_running.to_string();
                            let teams_count = d.teams_count.to_string();
                            let tools_count = d.tools_count.to_string();
                            let skills_count = d.skills_count.to_string();
                            view! {
                                <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
                                    <div class="card bg-base-100 shadow-xl">
                                        <div class="card-body">
                                            <span class="text-sm font-bold opacity-70">"Version"</span>
                                            <span class="stat-value-custom">{version}</span>
                                        </div>
                                    </div>
                                    <div class="card bg-base-100 shadow-xl">
                                        <div class="card-body">
                                            <span class="text-sm font-bold opacity-70">"Total Agents"</span>
                                            <span class="stat-value-custom">{agents_count}</span>
                                        </div>
                                    </div>
                                    <div class="card bg-base-100 shadow-xl border-l-4 border-primary">
                                        <div class="card-body">
                                            <span class="text-sm font-bold opacity-70">"Running"</span>
                                            <span class="stat-value-custom text-primary">{agents_running}</span>
                                        </div>
                                    </div>
                                    <div class="card bg-base-100 shadow-xl">
                                        <div class="card-body">
                                            <span class="text-sm font-bold opacity-70">"Teams"</span>
                                            <span class="stat-value-custom">{teams_count}</span>
                                        </div>
                                    </div>
                                    <div class="card bg-base-100 shadow-xl">
                                        <div class="card-body">
                                            <span class="text-sm font-bold opacity-70">"Tools"</span>
                                            <span class="stat-value-custom">{tools_count}</span>
                                        </div>
                                    </div>
                                    <div class="card bg-base-100 shadow-xl">
                                        <div class="card-body">
                                            <span class="text-sm font-bold opacity-70">"Skills"</span>
                                            <span class="stat-value-custom">{skills_count}</span>
                                        </div>
                                    </div>
                                </div>

                                <div class="flex items-center gap-2">
                                    <button class="btn btn-primary"
                                        on:click=move |_| {
                                            wasm_bindgen_futures::spawn_local(async {
                                                let _ = crate::api::post_empty("/freeze").await;
                                            });
                                        }
                                    >"Freeze All"</button>
                                    <button class="btn btn-secondary"
                                        on:click=move |_| {
                                            wasm_bindgen_futures::spawn_local(async {
                                                let _ = crate::api::post_empty("/revive").await;
                                            });
                                        }
                                    >"Revive All"</button>
                                </div>
                            }.into_any()
                        }
                        None => view! {
                            <div role="alert" class="alert alert-warning">
                                <span>"Could not connect to server"</span>
                            </div>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_data_deserialize() {
        let json = r#"{
            "agents_count": 3,
            "agents_running": 1,
            "sleeping_agents": 2,
            "teams_count": 1,
            "tools_count": 10,
            "skills_count": 5,
            "version": "0.1.0"
        }"#;
        let d: DashboardData = serde_json::from_str(json).unwrap();
        assert_eq!(d.agents_count, 3);
        assert_eq!(d.agents_running, 1);
        assert_eq!(d.teams_count, 1);
        assert_eq!(d.tools_count, 10);
        assert_eq!(d.skills_count, 5);
        assert_eq!(d.version, "0.1.0");
    }

    #[test]
    fn dashboard_data_ignores_extra_fields() {
        let json = r#"{
            "agents_count": 1,
            "agents_running": 0,
            "teams_count": 0,
            "tools_count": 0,
            "skills_count": 0,
            "version": "0.1.0",
            "sleeping_agents": 5,
            "unknown_field": true
        }"#;
        let d: DashboardData = serde_json::from_str(json).unwrap();
        assert_eq!(d.agents_count, 1);
    }
}
