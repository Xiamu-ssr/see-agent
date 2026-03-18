use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde::Deserialize;

use crate::api;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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

#[derive(Debug, Clone, Deserialize)]
struct TaskItem {
    id: String,
    title: String,
    description: String,
    status: String,
    assigned_to: Option<String>,
    #[allow(dead_code)]
    depends_on: Vec<String>,
    result: Option<String>,
    created_by: String,
    #[allow(dead_code)]
    created_at: String,
    #[allow(dead_code)]
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TeamMessage {
    #[allow(dead_code)]
    msg_id: Option<u64>,
    sender: String,
    content: String,
    priority: String,
    timestamp: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TeamTab {
    Members,
    Tasks,
    Messages,
}

// ---------------------------------------------------------------------------
// List page
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Detail page — tabbed view
// ---------------------------------------------------------------------------

#[component]
pub fn TeamDetail() -> impl IntoView {
    let params = use_params_map();
    let (tab, set_tab) = signal(TeamTab::Members);
    let tasks = RwSignal::new(Vec::<TaskItem>::new());
    let messages = RwSignal::new(Vec::<TeamMessage>::new());
    let (new_task_title, set_new_task_title) = signal(String::new());
    let (new_task_desc, set_new_task_desc) = signal(String::new());

    let team_id = Memo::new(move |_| {
        params.read().get("id").unwrap_or_default()
    });

    let team = LocalResource::new(move || {
        let id = team_id.get();
        async move {
            match id.is_empty() {
                true => None,
                false => api::get::<TeamSummary>(&format!("/teams/{id}")).await.ok(),
            }
        }
    });

    // Fetch tasks
    {
        wasm_bindgen_futures::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(100).await;
            let id = team_id.get_untracked();
            if !id.is_empty()
                && let Ok(t) = api::get::<Vec<TaskItem>>(&format!("/teams/{id}/tasks")).await
            {
                tasks.set(t);
            }
        });
    }

    // Fetch messages
    {
        wasm_bindgen_futures::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(100).await;
            let id = team_id.get_untracked();
            if !id.is_empty()
                && let Ok(m) =
                    api::get::<Vec<TeamMessage>>(&format!("/teams/{id}/messages")).await
            {
                messages.set(m);
            }
        });
    }

    // Create task handler
    let create_task = move |_| {
        let title = new_task_title.get();
        if title.trim().is_empty() {
            return;
        }
        let desc = new_task_desc.get();
        let id = team_id.get();
        set_new_task_title.set(String::new());
        set_new_task_desc.set(String::new());

        wasm_bindgen_futures::spawn_local(async move {
            let body = serde_json::json!({
                "title": title,
                "description": desc,
                "created_by": "user"
            });
            if let Ok(task) =
                api::post::<TaskItem>(&format!("/teams/{id}/tasks"), &body).await
            {
                tasks.update(|list| list.push(task));
            }
        });
    };

    // Update task status handler
    let update_task_status = move |task_id: String, new_status: String| {
        let id = team_id.get();
        tasks.update(|list| {
            if let Some(t) = list.iter_mut().find(|t| t.id == task_id) {
                t.status = new_status.clone();
            }
        });
        wasm_bindgen_futures::spawn_local(async move {
            let body = serde_json::json!({ "status": new_status });
            let _ = api::put::<TaskItem>(
                &format!("/teams/{id}/tasks/{task_id}"),
                &body,
            )
            .await;
        });
    };

    view! {
        <div class="page">
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || team.get().map(|t| {
                    match &*t {
                        Some(t) => {
                            let members = t.members.clone();
                            let team_name = t.name.clone();
                            let team_status = t.status.clone();

                            view! {
                                <div class="detail-header">
                                    <A href="/teams" attr:class="back-link">"< Teams"</A>
                                    <h2>{team_name}</h2>
                                    <span class="status-badge">{team_status}</span>
                                </div>

                                <div class="tabs">
                                    <button
                                        class=move || if tab.get() == TeamTab::Members { "tab active" } else { "tab" }
                                        on:click=move |_| set_tab.set(TeamTab::Members)
                                    >"Members"</button>
                                    <button
                                        class=move || if tab.get() == TeamTab::Tasks { "tab active" } else { "tab" }
                                        on:click=move |_| set_tab.set(TeamTab::Tasks)
                                    >"Task Board"</button>
                                    <button
                                        class=move || if tab.get() == TeamTab::Messages { "tab active" } else { "tab" }
                                        on:click=move |_| set_tab.set(TeamTab::Messages)
                                    >"Messages"</button>
                                </div>

                                <div class="tab-content">
                                    {move || match tab.get() {
                                        // ---------------------------------------------------
                                        // Members tab
                                        // ---------------------------------------------------
                                        TeamTab::Members => {
                                            let members = members.clone();
                                            view! {
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

                                        // ---------------------------------------------------
                                        // Task Board tab
                                        // ---------------------------------------------------
                                        TeamTab::Tasks => {
                                            view! {
                                                <div class="task-board">
                                                    // Create task form
                                                    <div class="task-create-form">
                                                        <input
                                                            type="text"
                                                            class="form-input"
                                                            placeholder="Task title..."
                                                            prop:value=new_task_title
                                                            on:input=move |ev| set_new_task_title.set(event_target_value(&ev))
                                                        />
                                                        <input
                                                            type="text"
                                                            class="form-input"
                                                            placeholder="Description (optional)"
                                                            prop:value=new_task_desc
                                                            on:input=move |ev| set_new_task_desc.set(event_target_value(&ev))
                                                        />
                                                        <button class="btn btn-primary btn-sm" on:click=create_task>"Create Task"</button>
                                                    </div>

                                                    // Task columns
                                                    <div class="task-columns">
                                                        {["pending", "claimed", "in_progress", "done"].into_iter().map(|status| {
                                                            let status_label = match status {
                                                                "pending" => "Pending",
                                                                "claimed" => "Claimed",
                                                                "in_progress" => "In Progress",
                                                                "done" => "Done",
                                                                _ => status,
                                                            };
                                                            let status_owned = status.to_string();
                                                            view! {
                                                                <div class="task-column">
                                                                    <h4 class="task-column-title">{status_label}</h4>
                                                                    {move || {
                                                                        let all_tasks = tasks.get();
                                                                        let filtered: Vec<_> = all_tasks.into_iter()
                                                                            .filter(|t| t.status == status_owned)
                                                                            .collect();

                                                                        if filtered.is_empty() {
                                                                            view! { <p class="empty task-empty">"—"</p> }.into_any()
                                                                        } else {
                                                                            filtered.into_iter().map(|task| {
                                                                                let task_id = task.id.clone();
                                                                                let next_status = match task.status.as_str() {
                                                                                    "pending" => Some("claimed"),
                                                                                    "claimed" => Some("in_progress"),
                                                                                    "in_progress" => Some("done"),
                                                                                    _ => None,
                                                                                };
                                                                                view! {
                                                                                    <div class="task-card">
                                                                                        <div class="task-title">{task.title}</div>
                                                                                        {if !task.description.is_empty() {
                                                                                            Some(view! { <div class="task-desc">{task.description}</div> })
                                                                                        } else { None }}
                                                                                        <div class="task-meta">
                                                                                            {task.assigned_to.map(|a| view! {
                                                                                                <span class="task-assignee">{a}</span>
                                                                                            })}
                                                                                            <span class="task-creator">{format!("by {}", task.created_by)}</span>
                                                                                        </div>
                                                                                        {task.result.map(|r| view! {
                                                                                            <div class="task-result">{r}</div>
                                                                                        })}
                                                                                        {next_status.map(|ns| {
                                                                                            let tid = task_id.clone();
                                                                                            let ns_str = ns.to_string();
                                                                                            let label = match ns {
                                                                                                "claimed" => "Claim",
                                                                                                "in_progress" => "Start",
                                                                                                "done" => "Complete",
                                                                                                _ => ns,
                                                                                            };
                                                                                            view! {
                                                                                                <button
                                                                                                    class="btn btn-sm task-action-btn"
                                                                                                    on:click=move |_| update_task_status(tid.clone(), ns_str.clone())
                                                                                                >{label}</button>
                                                                                            }
                                                                                        })}
                                                                                    </div>
                                                                                }
                                                                            }).collect_view().into_any()
                                                                        }
                                                                    }}
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            }.into_any()
                                        }

                                        // ---------------------------------------------------
                                        // Messages tab
                                        // ---------------------------------------------------
                                        TeamTab::Messages => {
                                            view! {
                                                <div class="team-messages">
                                                    {move || {
                                                        let msgs = messages.get();
                                                        if msgs.is_empty() {
                                                            view! { <p class="empty">"No messages yet"</p> }.into_any()
                                                        } else {
                                                            msgs.into_iter().map(|m| {
                                                                let priority_class = format!("msg-priority msg-{}", m.priority);
                                                                view! {
                                                                    <div class="team-msg">
                                                                        <div class="team-msg-header">
                                                                            <span class="team-msg-sender">{m.sender}</span>
                                                                            <span class=priority_class>{m.priority}</span>
                                                                            <span class="team-msg-time">{m.timestamp}</span>
                                                                        </div>
                                                                        <div class="team-msg-content">{m.content}</div>
                                                                    </div>
                                                                }
                                                            }).collect_view().into_any()
                                                        }
                                                    }}
                                                </div>
                                            }.into_any()
                                        }
                                    }}
                                </div>
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
