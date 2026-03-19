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
struct AgentSummary {
    id: String,
    name: String,
    emoji: String,
    state: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TaskItem {
    id: String,
    title: String,
    description: String,
    status: String,
    assigned_to: Option<String>,
    depends_on: Vec<String>,
    #[allow(dead_code)]
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

#[derive(Debug, Clone, Deserialize)]
struct FileEntry {
    name: String,
    #[serde(rename = "type")]
    entry_type: String,
    size: u64,
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
        <div class="h-full overflow-y-auto">
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
// Detail page — left: member list, right: tabs (Overview/TaskBoard/Messages/Shared)
// ---------------------------------------------------------------------------

#[component]
pub fn TeamDetail() -> impl IntoView {
    let params = use_params_map();
    let tasks = RwSignal::new(Vec::<TaskItem>::new());
    let messages = RwSignal::new(Vec::<TeamMessage>::new());
    let shared_files = RwSignal::new(Vec::<FileEntry>::new());
    let shared_path = RwSignal::new(String::new());
    let shared_file_content = RwSignal::new(Option::<String>::None);
    let agents = RwSignal::new(Vec::<AgentSummary>::new());
    let new_task_title = RwSignal::new(String::new());
    let new_task_desc = RwSignal::new(String::new());
    let active_tab = RwSignal::new("overview".to_string());

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

    // Fetch agents list for member info
    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(a) = api::get::<Vec<AgentSummary>>("/agents").await {
                agents.set(a);
            }
        });
    }

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

    // Fetch shared files
    {
        wasm_bindgen_futures::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(100).await;
            let id = team_id.get_untracked();
            if !id.is_empty()
                && let Ok(f) =
                    api::get::<Vec<FileEntry>>(&format!("/teams/{id}/files")).await
            {
                shared_files.set(f);
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
                            let team_id_val = t.id.clone();
                            let member_count = members.len();
                            let leader_id = members.iter().find(|m| m.role == "leader").map(|m| m.id.clone()).unwrap_or_default();

                            view! {
                                // --- Left sidebar: team members ---
                                <div class="w-56 shrink-0 border-r border-base-300 overflow-y-auto bg-base-200">
                                    <div class="p-3">
                                        <div class="flex items-center gap-2 mb-2">
                                            <A href="/teams">
                                                <button class="btn btn-ghost btn-xs">"\u{2190} Teams"</button>
                                            </A>
                                        </div>
                                        <h3 class="font-bold text-sm mb-1">{team_name.clone()}</h3>
                                        <span class="badge badge-info badge-sm">{team_status.clone()}</span>
                                    </div>
                                    <div class="divider my-0 mx-2"></div>
                                    <div class="p-2">
                                        <h4 class="text-xs font-bold opacity-70 px-2 mb-1">{format!("Members ({member_count})")}</h4>
                                        {members.clone().into_iter().map(|m| {
                                            let agent_href = format!("/agents/{}", m.id);
                                            let is_leader = m.role == "leader";
                                            let member_id = m.id.clone();
                                            let member_id2 = m.id.clone();
                                            let member_id3 = m.id.clone();
                                            let member_id4 = m.id.clone();
                                            let role = m.role.clone();
                                            view! {
                                                <A href=agent_href attr:class="block px-3 py-2 hover:bg-base-300 no-underline text-inherit transition-colors">
                                                    <div class="flex items-center gap-2">
                                                        // Emoji from agent info
                                                        <span class="text-sm">{move || {
                                                            let all = agents.get();
                                                            all.iter().find(|a| a.id == member_id).map(|a| a.emoji.clone()).unwrap_or_else(|| "\u{1F916}".to_string())
                                                        }}</span>
                                                        <div class="flex-1 min-w-0">
                                                            <div class="flex items-center gap-1">
                                                                {if is_leader { Some(view! { <span class="text-xs">{"\u{1F451}"}</span> }) } else { None }}
                                                                <span class="text-sm font-bold truncate">{move || {
                                                                    let all = agents.get();
                                                                    all.iter().find(|a| a.id == member_id2).map(|a| a.name.clone()).unwrap_or_else(|| member_id3.clone())
                                                                }}</span>
                                                            </div>
                                                            <div class="flex items-center gap-1">
                                                                <span class="text-xs opacity-50">{role}</span>
                                                                <span class="text-xs">{move || {
                                                                    let all = agents.get();
                                                                    let state = all.iter().find(|a| a.id == member_id4).map(|a| a.state.clone()).unwrap_or_default();
                                                                    if state == "active" { "\u{1F7E2}" } else { "\u{26AA}" }
                                                                }}</span>
                                                            </div>
                                                        </div>
                                                    </div>
                                                </A>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>

                                // --- Right panel: Tabs ---
                                <div class="flex-1 flex flex-col min-h-0 overflow-hidden">
                                    // Tab bar
                                    <div role="tablist" class="tabs tabs-bordered px-4 pt-2 shrink-0">
                                        {["overview", "taskboard", "messages", "shared"].into_iter().map(|tab| {
                                            let label = match tab {
                                                "overview" => "Overview",
                                                "taskboard" => "Task Board",
                                                "messages" => "Messages",
                                                "shared" => "Shared",
                                                _ => tab,
                                            };
                                            view! {
                                                <a role="tab"
                                                    class=move || if active_tab.get() == tab { "tab tab-active" } else { "tab" }
                                                    on:click=move |_| active_tab.set(tab.to_string())
                                                >{label}</a>
                                            }
                                        }).collect_view()}
                                    </div>

                                    // Tab content
                                    <div class="flex-1 overflow-y-auto p-4">
                                        {
                                            let team_name2 = team_name.clone();
                                            let team_status2 = team_status.clone();
                                            let team_id2 = team_id_val;
                                            let leader_id2 = leader_id;
                                            move || {
                                            let tab = active_tab.get();
                                            match tab.as_str() {
                                                // ============ Overview ============
                                                "overview" => {
                                                    let tn = team_name2.clone();
                                                    let ts = team_status2.clone();
                                                    let tid = team_id2.clone();
                                                    let lid = leader_id2.clone();
                                                    view! {
                                                        <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
                                                            <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                <span class="text-sm font-bold opacity-70">"Name"</span>
                                                                <p>{tn}</p>
                                                            </div></div>
                                                            <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                <span class="text-sm font-bold opacity-70">"ID"</span>
                                                                <p class="font-mono text-sm">{tid}</p>
                                                            </div></div>
                                                            <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                <span class="text-sm font-bold opacity-70">"Status"</span>
                                                                <span class="badge badge-info">{ts}</span>
                                                            </div></div>
                                                        </div>
                                                        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                                                            <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                <span class="text-sm font-bold opacity-70">"Leader"</span>
                                                                <p class="font-mono text-sm">{lid}</p>
                                                            </div></div>
                                                            <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                <span class="text-sm font-bold opacity-70">"Members"</span>
                                                                <p>{member_count}</p>
                                                            </div></div>
                                                            <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                <span class="text-sm font-bold opacity-70">"Tasks"</span>
                                                                {move || {
                                                                    let all = tasks.get();
                                                                    let pending = all.iter().filter(|t| t.status == "pending").count();
                                                                    let active = all.iter().filter(|t| t.status == "claimed" || t.status == "in_progress").count();
                                                                    let done = all.iter().filter(|t| t.status == "done").count();
                                                                    view! {
                                                                        <div class="flex gap-2">
                                                                            <span class="badge badge-ghost badge-sm">{format!("{pending} pending")}</span>
                                                                            <span class="badge badge-primary badge-sm">{format!("{active} active")}</span>
                                                                            <span class="badge badge-success badge-sm">{format!("{done} done")}</span>
                                                                        </div>
                                                                    }
                                                                }}
                                                            </div></div>
                                                        </div>
                                                    }.into_any()
                                                }

                                                // ============ Task Board ============
                                                "taskboard" => {
                                                    view! {
                                                        <div class="card bg-base-100 shadow-xl">
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
                                                                        // Build dependency tree: root tasks (no deps) + dependent tasks
                                                                        let task_ids: Vec<String> = all_tasks.iter().map(|t| t.id.clone()).collect();
                                                                        view! {
                                                                            <div class="space-y-2">
                                                                                {all_tasks.into_iter().map(|task| {
                                                                                    let task_id = task.id.clone();
                                                                                    let status_class = match task.status.as_str() {
                                                                                        "done" => "badge badge-success badge-sm",
                                                                                        "in_progress" => "badge badge-primary badge-sm",
                                                                                        "claimed" => "badge badge-info badge-sm",
                                                                                        _ => "badge badge-ghost badge-sm",
                                                                                    };
                                                                                    let next_status = match task.status.as_str() {
                                                                                        "pending" => Some(("claimed", "Claim")),
                                                                                        "claimed" => Some(("in_progress", "Start")),
                                                                                        "in_progress" => Some(("done", "Complete")),
                                                                                        _ => None,
                                                                                    };
                                                                                    let assignee = task.assigned_to.clone().unwrap_or_else(|| "\u{2014}".into());
                                                                                    let status_label = task.status.clone();
                                                                                    let has_deps = !task.depends_on.is_empty();
                                                                                    let deps = task.depends_on.clone();
                                                                                    let _task_ids = task_ids.clone();
                                                                                    view! {
                                                                                        <div class="card card-compact bg-base-200">
                                                                                            <div class="card-body p-3">
                                                                                                <div class="flex items-center justify-between">
                                                                                                    <div class="flex items-center gap-2 flex-1 min-w-0">
                                                                                                        <span class="font-bold text-sm truncate">{task.title}</span>
                                                                                                        <span class=status_class>{status_label}</span>
                                                                                                        <span class="text-xs opacity-50">{format!("#{}", task.id)}</span>
                                                                                                    </div>
                                                                                                    <div class="flex items-center gap-2">
                                                                                                        <span class="text-xs opacity-70">{assignee}</span>
                                                                                                        {next_status.map(|(ns, label)| {
                                                                                                            let tid = task_id.clone();
                                                                                                            let ns_str = ns.to_string();
                                                                                                            view! {
                                                                                                                <button class="btn btn-primary btn-xs"
                                                                                                                    on:click=move |_| update_task_status(tid.clone(), ns_str.clone())
                                                                                                                >{label}</button>
                                                                                                            }
                                                                                                        })}
                                                                                                    </div>
                                                                                                </div>
                                                                                                {if !task.description.is_empty() {
                                                                                                    Some(view! { <p class="text-xs opacity-70 mt-1">{task.description}</p> })
                                                                                                } else { None }}
                                                                                                {if has_deps {
                                                                                                    Some(view! {
                                                                                                        <div class="flex items-center gap-1 mt-1">
                                                                                                            <span class="text-xs opacity-50">"depends on:"</span>
                                                                                                            {deps.into_iter().map(|d| {
                                                                                                                view! { <span class="badge badge-outline badge-xs">{format!("#{d}")}</span> }
                                                                                                            }).collect_view()}
                                                                                                        </div>
                                                                                                    })
                                                                                                } else { None }}
                                                                                                {if !task.created_by.is_empty() {
                                                                                                    Some(view! {
                                                                                                        <span class="text-xs opacity-40">{format!("by {}", task.created_by)}</span>
                                                                                                    })
                                                                                                } else { None }}
                                                                                            </div>
                                                                                        </div>
                                                                                    }
                                                                                }).collect_view()}
                                                                            </div>
                                                                        }.into_any()
                                                                    }
                                                                }}
                                                            </div>
                                                        </div>
                                                    }.into_any()
                                                }

                                                // ============ Messages ============
                                                "messages" => {
                                                    view! {
                                                        <div class="card bg-base-100 shadow-xl">
                                                            <div class="card-body">
                                                                <h3 class="card-title text-sm">"Team Messages"</h3>
                                                                {move || {
                                                                    let msgs = messages.get();
                                                                    if msgs.is_empty() {
                                                                        view! { <span class="text-sm opacity-70">"No messages yet"</span> }.into_any()
                                                                    } else {
                                                                        msgs.into_iter().map(|m| {
                                                                            let priority_class = match m.priority.as_str() {
                                                                                "steer" => "badge badge-warning badge-xs",
                                                                                _ => "badge badge-primary badge-xs",
                                                                            };
                                                                            view! {
                                                                                <div class="py-2 border-b border-base-300">
                                                                                    <div class="flex justify-between items-center">
                                                                                        <span class="font-bold text-sm">{m.sender}</span>
                                                                                        <div class="flex items-center gap-2">
                                                                                            <span class=priority_class>{m.priority}</span>
                                                                                            <span class="text-xs opacity-70">{m.timestamp}</span>
                                                                                        </div>
                                                                                    </div>
                                                                                    <p class="text-sm mt-1">{m.content}</p>
                                                                                </div>
                                                                            }
                                                                        }).collect_view().into_any()
                                                                    }
                                                                }}
                                                            </div>
                                                        </div>
                                                    }.into_any()
                                                }

                                                // ============ Shared ============
                                                _ => {
                                                    let tid_shared = team_id2.clone();
                                                    let tid_shared2 = tid_shared.clone();
                                                    view! {
                                                        <div class="card bg-base-100 shadow-xl">
                                                            <div class="card-body">
                                                                <div class="flex items-center gap-2 mb-2">
                                                                    <h3 class="card-title text-sm">"Shared Files"</h3>
                                                                    {move || {
                                                                        let p = shared_path.get();
                                                                        if p.is_empty() {
                                                                            view! { <span class="text-xs opacity-50">"/shared/"</span> }.into_any()
                                                                        } else {
                                                                            let tid = tid_shared.clone();
                                                                            view! {
                                                                                <button class="btn btn-ghost btn-xs" on:click=move |_| {
                                                                                    // Go up one level
                                                                                    let current = shared_path.get_untracked();
                                                                                    let parent = current.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default();
                                                                                    shared_path.set(parent.clone());
                                                                                    shared_file_content.set(None);
                                                                                    let tid2 = tid.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        let url = if parent.is_empty() {
                                                                                            format!("/teams/{tid2}/files")
                                                                                        } else {
                                                                                            format!("/teams/{tid2}/files/{parent}")
                                                                                        };
                                                                                        if let Ok(f) = api::get::<Vec<FileEntry>>(&url).await {
                                                                                            shared_files.set(f);
                                                                                        }
                                                                                    });
                                                                                }>"\u{2190} Up"</button>
                                                                                <span class="text-xs opacity-50">{format!("/shared/{p}/")}</span>
                                                                            }.into_any()
                                                                        }
                                                                    }}
                                                                </div>
                                                                {move || {
                                                                    // Show file content if viewing a file
                                                                    if let Some(content) = shared_file_content.get() {
                                                                        return view! {
                                                                            <pre class="whitespace-pre-wrap text-sm bg-base-200 p-3 rounded overflow-auto max-h-96">{content}</pre>
                                                                        }.into_any();
                                                                    }
                                                                    let files = shared_files.get();
                                                                    if files.is_empty() {
                                                                        view! { <span class="text-sm opacity-70">"No shared files"</span> }.into_any()
                                                                    } else {
                                                                        view! {
                                                                            <div class="overflow-x-auto">
                                                                                <table class="table table-sm">
                                                                                    <thead>
                                                                                        <tr>
                                                                                            <th>"Name"</th>
                                                                                            <th>"Type"</th>
                                                                                            <th>"Size"</th>
                                                                                        </tr>
                                                                                    </thead>
                                                                                    <tbody>
                                                                                        {files.into_iter().map(|f| {
                                                                                            let icon = if f.entry_type == "directory" { "\u{1F4C1}" } else { "\u{1F4C4}" };
                                                                                            let is_dir = f.entry_type == "directory";
                                                                                            let fname = f.name.clone();
                                                                                            let fname2 = f.name.clone();
                                                                                            let tid = tid_shared2.clone();
                                                                                            view! {
                                                                                                <tr class="cursor-pointer hover:bg-base-200" on:click=move |_| {
                                                                                                    let current = shared_path.get_untracked();
                                                                                                    let new_path = if current.is_empty() {
                                                                                                        fname.clone()
                                                                                                    } else {
                                                                                                        format!("{current}/{}", fname)
                                                                                                    };
                                                                                                    if is_dir {
                                                                                                        shared_path.set(new_path.clone());
                                                                                                        shared_file_content.set(None);
                                                                                                        let tid2 = tid.clone();
                                                                                                        wasm_bindgen_futures::spawn_local(async move {
                                                                                                            if let Ok(f) = api::get::<Vec<FileEntry>>(&format!("/teams/{tid2}/files/{new_path}")).await {
                                                                                                                shared_files.set(f);
                                                                                                            }
                                                                                                        });
                                                                                                    } else {
                                                                                                        let tid2 = tid.clone();
                                                                                                        let np = new_path.clone();
                                                                                                        wasm_bindgen_futures::spawn_local(async move {
                                                                                                            if let Ok(content) = api::get_text(&format!("/teams/{tid2}/file/{np}")).await {
                                                                                                                shared_file_content.set(Some(content));
                                                                                                            }
                                                                                                        });
                                                                                                    }
                                                                                                }>
                                                                                                    <td><span>{icon}" "{fname2}</span></td>
                                                                                                    <td><span class="text-xs opacity-70">{f.entry_type}</span></td>
                                                                                                    <td><span class="text-xs opacity-70">{format!("{}", f.size)}</span></td>
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
                                                    }.into_any()
                                                }
                                            }
                                        }}
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
