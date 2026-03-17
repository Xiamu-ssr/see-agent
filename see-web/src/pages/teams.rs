use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct TeamSummary {
    id: String,
    name: String,
    status: String,
    members: Vec<String>,
}

#[component]
pub fn Teams() -> impl IntoView {
    let teams = LocalResource::new(|| async {
        api::get::<Vec<TeamSummary>>("/teams").await.unwrap_or_default()
    });

    view! {
        <div class="page">
            <h2>"Teams"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || teams.get().map(|list| {
                    let items: Vec<_> = list.iter().cloned().collect();
                    view! {
                        <div class="card-grid">
                            {items.into_iter().map(|t| {
                                let href = format!("/teams/{}", t.id);
                                let count = t.members.len();
                                view! {
                                    <A href=href attr:class="card">
                                        <span class="card-name">{t.name}</span>
                                        <span class="card-status">{t.status}</span>
                                        <span class="card-meta">{format!("{count} members")}</span>
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
struct TeamDetailData {
    #[allow(dead_code)]
    id: String,
    name: String,
    status: String,
    leader: Option<String>,
    members: Vec<MemberInfo>,
}

#[derive(Debug, Clone, Deserialize)]
struct MemberInfo {
    id: String,
    role: String,
}

#[component]
pub fn TeamDetail() -> impl IntoView {
    let params = use_params_map();

    let team = LocalResource::new(move || {
        let id = params.read().get("id");
        async move {
            match id {
                Some(id) => api::get::<TeamDetailData>(&format!("/teams/{id}")).await.ok(),
                None => None,
            }
        }
    });

    view! {
        <div class="page">
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || team.get().map(|t| {
                    match &*t {
                        Some(t) => {
                            let members: Vec<_> = t.members.clone();
                            view! {
                            <h2>{t.name.clone()}</h2>
                            <p>"Status: " {t.status.clone()}</p>
                            {t.leader.clone().map(|l| view! { <p>"Leader: " {l}</p> })}
                            <h3>"Members"</h3>
                            <ul>
                                {members.into_iter().map(|m| {
                                    view! { <li>{m.id} " — " {m.role}</li> }
                                }).collect_view()}
                            </ul>
                        }.into_any()
                        }
                        None => view! {
                            <p class="error">"Team not found"</p>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
