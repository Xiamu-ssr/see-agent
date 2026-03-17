use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct AgentSummary {
    id: String,
    name: String,
    emoji: String,
    status: String,
    #[allow(dead_code)]
    team_id: Option<String>,
    team_name: Option<String>,
}

#[component]
pub fn Agents() -> impl IntoView {
    let agents = LocalResource::new(|| async {
        api::get::<Vec<AgentSummary>>("/agents").await.unwrap_or_default()
    });

    view! {
        <div class="page">
            <h2>"Agents"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || agents.get().map(|list| {
                    let items: Vec<_> = list.iter().cloned().collect();
                    view! {
                        <div class="card-grid">
                            {items.into_iter().map(|a| {
                                let href = format!("/agents/{}", a.id);
                                view! {
                                    <A href=href attr:class="card">
                                        <span class="card-emoji">{a.emoji}</span>
                                        <span class="card-name">{a.name}</span>
                                        <span class="card-status">{a.status}</span>
                                        {a.team_name.map(|t| view! {
                                            <span class="card-team">{t}</span>
                                        })}
                                    </A>
                                }
                            }).collect_view()}
                        </div>
                    }
                })}
            </Suspense>
        </div>
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AgentDetailData {
    #[allow(dead_code)]
    id: String,
    name: String,
    emoji: String,
    status: String,
    system_prompt_preview: Option<String>,
}

#[component]
pub fn AgentDetail() -> impl IntoView {
    let params = use_params_map();

    let agent = LocalResource::new(move || {
        let id = params.read().get("id");
        async move {
            match id {
                Some(id) => api::get::<AgentDetailData>(&format!("/agents/{id}")).await.ok(),
                None => None,
            }
        }
    });

    view! {
        <div class="page">
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || agent.get().map(|a| {
                    match &*a {
                        Some(a) => view! {
                            <div class="detail-header">
                                <span class="detail-emoji">{a.emoji.clone()}</span>
                                <h2>{a.name.clone()}</h2>
                                <span class="detail-status">{a.status.clone()}</span>
                            </div>
                            <div class="detail-body">
                                <section>
                                    <h3>"System Prompt"</h3>
                                    <pre>{a.system_prompt_preview.clone().unwrap_or_default()}</pre>
                                </section>
                            </div>
                        }.into_any(),
                        None => view! {
                            <p class="error">"Agent not found"</p>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
