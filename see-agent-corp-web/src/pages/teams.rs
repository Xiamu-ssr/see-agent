use leptos::ev;
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
    #[allow(dead_code)]
    description: String,
    status: String,
    assigned_to: Option<String>,
    #[allow(dead_code)]
    depends_on: Vec<String>,
    #[allow(dead_code)]
    result: Option<String>,
    #[allow(dead_code)]
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

// ---------------------------------------------------------------------------
// List page
// ---------------------------------------------------------------------------

#[component]
pub fn Teams() -> impl IntoView {
    let teams = LocalResource::new(|| async {
        api::get::<Vec<TeamSummary>>("/teams").await.unwrap_or_default()
    });

    view! {
        <div>
            <h2 class="text-xl font-bold mb-4">"Teams"</h2>
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || teams.get().map(|list| {
                    let items: Vec<_> = list.iter().cloned().collect();
                    if items.is_empty() {
                        view! {
                            <div class="text-center py-12 opacity-60">
                                <p class="text-4xl mb-2">"👥"</p>
                                <p>"No teams yet"</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                                {items.into_iter().map(|t| {
                                    let href = format!("/teams/{}", t.id);
                                    let count = t.members.len();
                                    view! {
                                        <A href=href attr:class="no-underline text-inherit">
                                            <div class="card bg-base-100 shadow-xl hover:shadow-2xl transition-shadow cursor-pointer">
                                                <div class="card-body">
                                                    <h3 class="card-title text-sm">{t.name}</h3>
                                                    <span class="badge badge-info">{t.status}</span>
                                                    <span class="text-sm opacity-70">{format!("{count} members")}</span>
                                                </div>
                                            </div>
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
// Detail page — master-detail: members sidebar + task board & messages
// ---------------------------------------------------------------------------

#[component]
pub fn TeamDetail() -> impl IntoView {
    let params = use_params_map();
    let tasks = RwSignal::new(Vec::<TaskItem>::new());
    let messages = RwSignal::new(Vec::<TeamMessage>::new());
    let new_task_title = RwSignal::new(String::new());
    let new_task_desc = RwSignal::new(String::new());

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
        new_task_title.set(String::new());
        new_task_desc.set(String::new());

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
        <div class="flex h-full min-h-0">
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || team.get().map(|t| {
                    match &*t {
                        Some(t) => {
                            let members = t.members.clone();
                            let team_name = t.name.clone();
                            let team_status = t.status.clone();

                            view! {
                                // --- Left sidebar: team members ---
                                <div class="w-56 shrink-0 border-r border-base-300 overflow-y-auto bg-base-200">
                                    <div class="p-3">
                                        <div class="flex items-center gap-2 mb-2">
                                            <A href="/teams">
                                                <button class="btn btn-ghost btn-xs">"< Teams"</button>
                                            </A>
                                        </div>
                                        <h3 class="font-bold text-sm mb-1">{team_name}</h3>
                                        <span class="badge badge-info badge-sm">{team_status}</span>
                                    </div>
                                    <div class="divider my-0 mx-2"></div>
                                    <div class="p-2">
                                        <h4 class="text-xs font-bold opacity-70 px-2 mb-1">"Members"</h4>
                                        {members.into_iter().map(|m| {
                                            let agent_href = format!("/agents/{}", m.id);
                                            let role_class = if m.role == "leader" { "badge badge-warning badge-xs" } else { "badge badge-ghost badge-xs" };
                                            view! {
                                                <A href=agent_href attr:class="block px-3 py-2 hover:bg-base-300 no-underline text-inherit transition-colors">
                                                    <div class="flex items-center justify-between">
                                                        <span class="text-sm font-bold truncate">{m.id}</span>
                                                        <span class=role_class>{m.role}</span>
                                                    </div>
                                                </A>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>

                                // --- Right panel: Task Board + Messages ---
                                <div class="flex-1 flex flex-col min-h-0 overflow-y-auto p-4">
                                    // --- Task Board ---
                                    <div class="card bg-base-100 shadow-xl mb-4">
                                        <div class="card-body">
                                            <h3 class="card-title text-sm">"Task Board"</h3>
                                            <div class="flex items-center gap-2">
                                                <input class="input input-bordered input-sm flex-1"
                                                    placeholder="Task title..."
                                                    prop:value=move || new_task_title.get()
                                                    on:input=move |ev: ev::Event| new_task_title.set(event_target_value(&ev))
                                                />
                                                <input class="input input-bordered input-sm flex-1"
                                                    placeholder="Description (optional)"
                                                    prop:value=move || new_task_desc.get()
                                                    on:input=move |ev: ev::Event| new_task_desc.set(event_target_value(&ev))
                                                />
                                                <button class="btn btn-primary btn-sm" on:click=create_task>"Create"</button>
                                            </div>
                                            <div class="divider my-1"></div>
                                            {move || {
                                                let all_tasks = tasks.get();
                                                if all_tasks.is_empty() {
                                                    view! { <span class="text-sm opacity-70">"No tasks yet"</span> }.into_any()
                                                } else {
                                                    view! {
                                                        <div class="overflow-x-auto">
                                                            <table class="table">
                                                                <thead>
                                                                    <tr>
                                                                        <th>"Title"</th>
                                                                        <th>"Status"</th>
                                                                        <th>"Assignee"</th>
                                                                        <th>"Action"</th>
                                                                    </tr>
                                                                </thead>
                                                                <tbody>
                                                                    {all_tasks.into_iter().map(|task| {
                                                                        let task_id = task.id.clone();
                                                                        let status_class = match task.status.as_str() {
                                                                            "done" => "badge badge-success",
                                                                            "in_progress" => "badge badge-primary",
                                                                            "claimed" => "badge badge-info",
                                                                            _ => "badge badge-ghost",
                                                                        };
                                                                        let next_status = match task.status.as_str() {
                                                                            "pending" => Some(("claimed", "Claim")),
                                                                            "claimed" => Some(("in_progress", "Start")),
                                                                            "in_progress" => Some(("done", "Complete")),
                                                                            _ => None,
                                                                        };
                                                                        let assignee = task.assigned_to.unwrap_or_else(|| "\u{2014}".into());
                                                                        let status_label = task.status;
                                                                        view! {
                                                                            <tr>
                                                                                <td>{task.title}</td>
                                                                                <td><span class=status_class>{status_label}</span></td>
                                                                                <td>{assignee}</td>
                                                                                <td>
                                                                                    {next_status.map(|(ns, label)| {
                                                                                        let tid = task_id.clone();
                                                                                        let ns_str = ns.to_string();
                                                                                        view! {
                                                                                            <button class="btn btn-primary btn-xs"
                                                                                                on:click=move |_| update_task_status(tid.clone(), ns_str.clone())
                                                                                            >{label}</button>
                                                                                        }
                                                                                    })}
                                                                                </td>
                                                                            </tr>
                                                                        }
                                                                    }).collect_view()}
                                                                </tbody>
                                                            </table>
                                                        </div>
                                                    }.into_any()
                                                }
                                            }}
                                        </div>
                                    </div>

                                    // --- Messages ---
                                    <div class="card bg-base-100 shadow-xl">
                                        <div class="card-body">
                                            <h3 class="card-title text-sm">"Messages"</h3>
                                            {move || {
                                                let msgs = messages.get();
                                                if msgs.is_empty() {
                                                    view! { <span class="text-sm opacity-70">"No messages yet"</span> }.into_any()
                                                } else {
                                                    msgs.into_iter().map(|m| {
                                                        let priority_class = match m.priority.as_str() {
                                                            "steer" => "badge badge-warning",
                                                            _ => "badge badge-primary",
                                                        };
                                                        view! {
                                                            <div class="py-2 border-b border-base-300">
                                                                <div class="flex justify-between items-center">
                                                                    <span class="font-bold">{m.sender}</span>
                                                                    <div class="flex items-center gap-2">
                                                                        <span class=priority_class>{m.priority}</span>
                                                                        <span class="text-sm opacity-70">{m.timestamp}</span>
                                                                    </div>
                                                                </div>
                                                                <p>{m.content}</p>
                                                            </div>
                                                        }
                                                    }).collect_view().into_any()
                                                }
                                            }}
                                        </div>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        None => view! {
                            <div role="alert" class="alert alert-error">
                                <span>"Team not found"</span>
                            </div>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
