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
    members: Vec<MemberInfo>,
}

#[derive(Debug, Clone, Deserialize)]
struct MemberInfo {
    id: String,
    role: String,
}

#[component]
pub fn Teams() -> impl IntoView {
    let teams = LocalResource::new(|| async {
        api::get::<Vec<TeamSummary>>("/teams").await.unwrap_or_default()
    });

    view! {
        <div class="page">
            <div class="page-header">
                <h2>"Teams"</h2>
            </div>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || teams.get().map(|list| {
                    let items: Vec<_> = list.iter().cloned().collect();
                    if items.is_empty() {
                        view! { <p class="empty">"No teams yet"</p> }.into_any()
                    } else {
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
                        }.into_any()
                    }
                })}
            </Suspense>
        </div>
    }
}

#[component]
pub fn TeamDetail() -> impl IntoView {
    let params = use_params_map();

    let team = LocalResource::new(move || {
        let id = params.read().get("id");
        async move {
            match id {
                Some(id) => api::get::<TeamSummary>(&format!("/teams/{id}")).await.ok(),
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
                            let members = t.members.clone();
                            view! {
                                <div class="detail-header">
                                    <A href="/teams" attr:class="back-link">"< Teams"</A>
                                    <h2>{t.name.clone()}</h2>
                                    <span class="status-badge">{t.status.clone()}</span>
                                </div>
                                <h3>"Members"</h3>
                                <table class="data-table">
                                    <thead>
                                        <tr>
                                            <th>"Agent ID"</th>
                                            <th>"Role"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {members.into_iter().map(|m| {
                                            let agent_href = format!("/agents/{}", m.id);
                                            view! {
                                                <tr>
                                                    <td><A href=agent_href>{m.id}</A></td>
                                                    <td>{m.role}</td>
                                                </tr>
                                            }
                                        }).collect_view()}
                                    </tbody>
                                </table>
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
