use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde::Deserialize;

use crate::api;

// ---------------------------------------------------------------------------
// List page
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct AgentSummary {
    id: String,
    name: String,
    emoji: String,
    status: String,
    #[allow(dead_code)]
    team_id: Option<String>,
}

#[component]
pub fn Agents() -> impl IntoView {
    let agents = LocalResource::new(|| async {
        api::get::<Vec<AgentSummary>>("/agents").await.unwrap_or_default()
    });

    view! {
        <div class="page">
            <div class="page-header">
                <h2>"Agents"</h2>
            </div>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || agents.get().map(|list| {
                    let items: Vec<_> = list.iter().cloned().collect();
                    if items.is_empty() {
                        view! { <p class="empty">"No agents yet"</p> }.into_any()
                    } else {
                        view! {
                            <div class="card-grid">
                                {items.into_iter().map(|a| {
                                    let href = format!("/agents/{}", a.id);
                                    let status_class = format!("status-badge status-{}", a.status);
                                    view! {
                                        <A href=href attr:class="card">
                                            <span class="card-emoji">{a.emoji}</span>
                                            <span class="card-name">{a.name}</span>
                                            <span class=status_class>{a.status}</span>
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

// ---------------------------------------------------------------------------
// Detail page — tabbed view
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct AgentDetailData {
    id: String,
    name: String,
    emoji: String,
    status: String,
    tools: Vec<String>,
    skills: Vec<String>,
    has_soul: bool,
    location: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Chat,
    Files,
    Tools,
    Skills,
}

#[component]
pub fn AgentDetail() -> impl IntoView {
    let params = use_params_map();
    let (tab, set_tab) = signal(Tab::Overview);
    let (msg_input, set_msg_input) = signal(String::new());
    let (chat_log, set_chat_log) = signal::<Vec<ChatEntry>>(Vec::new());

    let agent_id = Memo::new(move |_| {
        params.read().get("id").unwrap_or_default()
    });

    let agent = LocalResource::new(move || {
        let id = agent_id.get();
        async move {
            if id.is_empty() {
                None
            } else {
                api::get::<AgentDetailData>(&format!("/agents/{id}")).await.ok()
            }
        }
    });

    let send_msg = move || {
        let content = msg_input.get();
        if content.trim().is_empty() {
            return;
        }
        let id = agent_id.get();
        set_chat_log.update(|log| {
            log.push(ChatEntry {
                sender: "you".into(),
                content: content.clone(),
            });
        });
        set_msg_input.set(String::new());

        wasm_bindgen_futures::spawn_local(async move {
            let body = serde_json::json!({
                "content": content,
                "priority": "collect"
            });
            let _ = api::post::<serde_json::Value>(&format!("/agents/{id}/message"), &body).await;
        });
    };

    view! {
        <div class="page">
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || agent.get().map(|a| {
                    match &*a {
                        Some(a) => {
                            let id = a.id.clone();
                            let name = a.name.clone();
                            let emoji = a.emoji.clone();
                            let status = a.status.clone();
                            let tools = a.tools.clone();
                            let skills = a.skills.clone();
                            let has_soul = a.has_soul;
                            let location = a.location.clone();
                            let status_class = format!("status-badge status-{status}");

                            view! {
                                <div class="detail-header">
                                    <A href="/agents" attr:class="back-link">"< Agents"</A>
                                    <span class="detail-emoji">{emoji}</span>
                                    <h2>{name}</h2>
                                    <span class=status_class>{status.clone()}</span>
                                </div>

                                <div class="tabs">
                                    <button
                                        class=move || if tab.get() == Tab::Overview { "tab active" } else { "tab" }
                                        on:click=move |_| set_tab.set(Tab::Overview)
                                    >"Overview"</button>
                                    <button
                                        class=move || if tab.get() == Tab::Chat { "tab active" } else { "tab" }
                                        on:click=move |_| set_tab.set(Tab::Chat)
                                    >"Chat"</button>
                                    <button
                                        class=move || if tab.get() == Tab::Files { "tab active" } else { "tab" }
                                        on:click=move |_| set_tab.set(Tab::Files)
                                    >"Files"</button>
                                    <button
                                        class=move || if tab.get() == Tab::Tools { "tab active" } else { "tab" }
                                        on:click=move |_| set_tab.set(Tab::Tools)
                                    >"Tools"</button>
                                    <button
                                        class=move || if tab.get() == Tab::Skills { "tab active" } else { "tab" }
                                        on:click=move |_| set_tab.set(Tab::Skills)
                                    >"Skills"</button>
                                </div>

                                <div class="tab-content">
                                    {move || match tab.get() {
                                        Tab::Overview => view! {
                                            <div class="overview-grid">
                                                <div class="info-card">
                                                    <span class="info-label">"ID"</span>
                                                    <span class="info-value">{id.clone()}</span>
                                                </div>
                                                <div class="info-card">
                                                    <span class="info-label">"Status"</span>
                                                    <span class="info-value">{status.clone()}</span>
                                                </div>
                                                <div class="info-card">
                                                    <span class="info-label">"Has SOUL.md"</span>
                                                    <span class="info-value">{if has_soul { "Yes" } else { "No" }}</span>
                                                </div>
                                                <div class="info-card">
                                                    <span class="info-label">"Location"</span>
                                                    <span class="info-value mono">{location.clone()}</span>
                                                </div>
                                                <div class="info-card">
                                                    <span class="info-label">"Tools"</span>
                                                    <span class="info-value">{tools.len()}</span>
                                                </div>
                                                <div class="info-card">
                                                    <span class="info-label">"Skills"</span>
                                                    <span class="info-value">{skills.len()}</span>
                                                </div>
                                            </div>
                                        }.into_any(),

                                        Tab::Chat => {
                                            let send = send_msg;
                                            view! {
                                                <div class="chat-panel">
                                                    <div class="chat-log">
                                                        {move || {
                                                            let entries = chat_log.get();
                                                            entries.into_iter().map(|e| {
                                                                let cls = format!("chat-msg chat-{}", e.sender);
                                                                view! {
                                                                    <div class=cls>
                                                                        <span class="chat-sender">{e.sender}</span>
                                                                        <span class="chat-content">{e.content}</span>
                                                                    </div>
                                                                }
                                                            }).collect_view()
                                                        }}
                                                    </div>
                                                    <div class="chat-input-row">
                                                        <input
                                                            type="text"
                                                            class="chat-input"
                                                            placeholder="Send a message..."
                                                            prop:value=msg_input
                                                            on:input=move |ev| {
                                                                set_msg_input.set(event_target_value(&ev));
                                                            }
                                                            on:keydown=move |ev| {
                                                                if ev.key() == "Enter" {
                                                                    send();
                                                                }
                                                            }
                                                        />
                                                        <button
                                                            class="btn btn-primary"
                                                            on:click=move |_| (send_msg)()
                                                        >"Send"</button>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        }

                                        Tab::Files => view! {
                                            <p class="empty">"File browser coming in Phase 16+"</p>
                                        }.into_any(),

                                        Tab::Tools => {
                                            let tl = tools.clone();
                                            if tl.is_empty() {
                                                view! { <p class="empty">"No tools loaded"</p> }.into_any()
                                            } else {
                                                view! {
                                                    <ul class="simple-list">
                                                        {tl.into_iter().map(|t| view! { <li>{t}</li> }).collect_view()}
                                                    </ul>
                                                }.into_any()
                                            }
                                        }

                                        Tab::Skills => {
                                            let sl = skills.clone();
                                            if sl.is_empty() {
                                                view! { <p class="empty">"No skills loaded"</p> }.into_any()
                                            } else {
                                                view! {
                                                    <ul class="simple-list">
                                                        {sl.into_iter().map(|s| view! { <li>{s}</li> }).collect_view()}
                                                    </ul>
                                                }.into_any()
                                            }
                                        }
                                    }}
                                </div>
                            }.into_any()
                        }
                        None => view! {
                            <p class="error">"Agent not found"</p>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}

#[derive(Debug, Clone)]
struct ChatEntry {
    sender: String,
    content: String,
}
