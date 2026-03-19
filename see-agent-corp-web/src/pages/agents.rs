use leptos::ev;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde::Deserialize;

use crate::api;
use crate::components::markdown::render_markdown;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct AgentSummary {
    id: String,
    name: String,
    emoji: String,
    state: String,
    #[allow(dead_code)]
    team_id: Option<String>,
    #[serde(default)]
    is_system: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentDetailData {
    id: String,
    name: String,
    emoji: String,
    state: String,
    #[allow(dead_code)]
    tools: Vec<String>,
    #[allow(dead_code)]
    skills: Vec<String>,
    has_soul: bool,
    location: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SessionMsg {
    #[allow(dead_code)]
    msg_id: u64,
    #[allow(dead_code)]
    timestamp: String,
    msg_type: String,
    data: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
struct ToolInfo {
    name: String,
    description: String,
    disabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct SkillInfo {
    name: String,
    description: String,
    available: bool,
    #[allow(dead_code)]
    disabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct FileEntry {
    name: String,
    #[serde(rename = "type")]
    entry_type: String,
    size: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_message_text(msg: &SessionMsg) -> String {
    if let Some(content) = msg.data.get("content") {
        if let Some(s) = content.as_str() {
            return s.to_string();
        }
        if let Some(parts) = content.as_array() {
            return parts
                .iter()
                .filter_map(|p| {
                    if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                        p.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
    if let Some(text) = msg.data.get("text").and_then(|t| t.as_str()) {
        return text.to_string();
    }
    if msg.data.is_string() {
        return msg.data.as_str().unwrap_or("").to_string();
    }
    serde_json::to_string_pretty(&msg.data).unwrap_or_default()
}

fn format_file_size(size: u64) -> String {
    if size < 1024 {
        format!("{size} B")
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    }
}

fn status_badge_class(status: &str) -> &'static str {
    match status {
        "running" => "badge badge-success",
        "idle" => "badge badge-ghost",
        "error" => "badge badge-error",
        _ => "badge badge-info",
    }
}

// ---------------------------------------------------------------------------
// Master-detail page: agent list sidebar + agent detail panel
// ---------------------------------------------------------------------------

#[component]
pub fn AgentsPage() -> impl IntoView {
    let params = use_params_map();

    let agent_id = Memo::new(move |_| {
        params.read().get("id").unwrap_or_default()
    });

    // Fetch agent list
    let agents = LocalResource::new(|| async {
        api::get::<Vec<AgentSummary>>("/agents").await.unwrap_or_default()
    });

    view! {
        <div class="flex h-full min-h-0">
            // --- Left sidebar: agent list ---
            <div class="w-56 shrink-0 border-r border-base-300 overflow-y-auto bg-base-200">
                <div class="p-2">
                    <h3 class="text-sm font-bold opacity-70 px-2 mb-2">"Agents"</h3>
                </div>
                <Suspense fallback=|| view! { <span class="loading loading-spinner loading-sm m-4"></span> }>
                    {move || agents.get().map(|list| {
                        let items: Vec<_> = list.iter().filter(|a| !a.is_system).cloned().collect();
                        if items.is_empty() {
                            view! {
                                <div class="text-center py-4 opacity-60 text-sm">"No agents"</div>
                            }.into_any()
                        } else {
                            items.into_iter().map(|a| {
                                let href = format!("/agents/{}", a.id);
                                let aid = a.id.clone();
                                let badge_class = status_badge_class(&a.state);
                                view! {
                                    <A href=href attr:class=move || {
                                        let base = "block px-3 py-2 hover:bg-base-300 no-underline text-inherit transition-colors";
                                        if agent_id.get() == aid { format!("{base} bg-base-300 border-l-2 border-primary") } else { base.to_string() }
                                    }>
                                        <div class="flex items-center gap-2">
                                            <span>{a.emoji}</span>
                                            <div class="flex-1 min-w-0">
                                                <div class="text-sm font-bold truncate">{a.name}</div>
                                                <span class=format!("{badge_class} badge-xs")>{a.state}</span>
                                            </div>
                                        </div>
                                    </A>
                                }
                            }).collect_view().into_any()
                        }
                    })}
                </Suspense>
                // System agent entry at bottom
                <div class="border-t border-base-300 mt-auto">
                    <A href="/agents/system" attr:class=move || {
                        let base = "block px-3 py-2 hover:bg-base-300 no-underline text-inherit transition-colors";
                        if agent_id.get() == "system" { format!("{base} bg-base-300 border-l-2 border-primary") } else { base.to_string() }
                    }>
                        <div class="flex items-center gap-2">
                            <span>"⚙️"</span>
                            <div class="text-sm font-bold">"System"</div>
                        </div>
                    </A>
                </div>
            </div>

            // --- Right panel: agent detail ---
            <div class="flex-1 flex flex-col min-h-0 overflow-y-auto p-4">
                {move || {
                    let id = agent_id.get();
                    if id.is_empty() {
                        view! {
                            <div class="flex items-center justify-center h-full opacity-50">
                                <div class="text-center">
                                    <p class="text-4xl mb-2">"<-"</p>
                                    <p>"Select an agent from the list"</p>
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! { <AgentDetailPanel agent_id=id /> }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Detail panel (extracted from the old AgentDetail)
// ---------------------------------------------------------------------------

#[component]
fn AgentDetailPanel(agent_id: String) -> impl IntoView {
    // "chat" or "details"
    let view_mode = RwSignal::new(String::from("chat"));
    let details_tab = RwSignal::new(String::from("info"));
    let msg_input = RwSignal::new(String::new());
    let msg_priority = RwSignal::new(String::from("collect"));
    let chat_messages = RwSignal::new(Vec::<SessionMsg>::new());
    let tools_list = RwSignal::new(Vec::<ToolInfo>::new());
    let skills_list = RwSignal::new(Vec::<SkillInfo>::new());
    let file_entries = RwSignal::new(Vec::<FileEntry>::new());
    let file_path = RwSignal::new(String::new());
    let open_file_full_path = RwSignal::new(String::new());
    let file_content = RwSignal::new(Option::<String>::None);
    let file_edit_text = RwSignal::new(String::new());
    let is_editing_file = RwSignal::new(false);
    let agent_status = RwSignal::new("idle".to_string());
    let is_active = RwSignal::new(true);
    let agent_logs = RwSignal::new(Vec::<String>::new());

    let aid = StoredValue::new(agent_id.clone());

    let agent = LocalResource::new({
        let id = agent_id.clone();
        move || {
            let id = id.clone();
            async move {
                let result = api::get::<AgentDetailData>(&format!("/agents/{id}")).await.ok();
                if let Some(ref data) = result {
                    agent_status.set(data.state.clone());
                }
                result
            }
        }
    });

    // --- Polling for chat messages ---
    on_cleanup(move || is_active.set(false));
    {
        let id = agent_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(2000).await;
                if !is_active.get_untracked() {
                    break;
                }
                if let Ok(msgs) =
                    api::get::<Vec<SessionMsg>>(&format!("/agents/{id}/session/messages")).await
                {
                    chat_messages.set(msgs);
                }
            }
        });
    }

    // --- Fetch tools ---
    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(tools) = api::get::<Vec<ToolInfo>>("/tools").await {
                tools_list.set(tools);
            }
        });
    }

    // --- Fetch skills ---
    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(skills) = api::get::<Vec<SkillInfo>>("/skills").await {
                skills_list.set(skills);
            }
        });
    }

    // --- Polling for agent logs ---
    {
        let id = agent_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(5000).await;
                if !is_active.get_untracked() {
                    break;
                }
                if let Ok(lines) =
                    api::get::<Vec<String>>(&format!("/agents/{id}/logs")).await
                {
                    agent_logs.set(lines);
                }
            }
        });
    }

    // --- Fetch root files ---
    {
        let id = agent_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(100).await;
            if let Ok(entries) =
                api::get::<Vec<FileEntry>>(&format!("/agents/{id}/files")).await
            {
                file_entries.set(entries);
            }
        });
    }

    // --- Chat handlers ---
    let send_msg = move || {
        let content = msg_input.get();
        if content.trim().is_empty() {
            return;
        }
        let id = aid.get_value();
        let priority = msg_priority.get();
        msg_input.set(String::new());

        wasm_bindgen_futures::spawn_local(async move {
            let body = serde_json::json!({
                "content": content,
                "priority": priority
            });
            let _ =
                api::post::<serde_json::Value>(&format!("/agents/{id}/message"), &body).await;
        });
    };

    // --- Tool toggle handler ---
    let toggle_tool = move |name: String, new_disabled: bool| {
        let id = aid.get_value();
        let body = serde_json::json!({ "disabled": new_disabled });
        tools_list.update(|list| {
            if let Some(tool) = list.iter_mut().find(|t| t.name == name) {
                tool.disabled = new_disabled;
            }
        });
        wasm_bindgen_futures::spawn_local(async move {
            let _ = api::post::<serde_json::Value>(
                &format!("/agents/{id}/tools/{name}/toggle"),
                &body,
            )
            .await;
        });
    };

    // --- File handlers ---
    let fetch_dir = move |path: String| {
        let id = aid.get_value();
        let fetch_path = path.clone();
        file_path.set(path);
        file_content.set(None);
        is_editing_file.set(false);

        wasm_bindgen_futures::spawn_local(async move {
            let url = if fetch_path.is_empty() {
                format!("/agents/{id}/files")
            } else {
                format!("/agents/{id}/files/{fetch_path}")
            };
            if let Ok(entries) = api::get::<Vec<FileEntry>>(&url).await {
                file_entries.set(entries);
            }
        });
    };

    let open_file = move |file_name: String| {
        let id = aid.get_value();
        let current = file_path.get_untracked();
        let full_path = if current.is_empty() {
            file_name
        } else {
            format!("{current}/{file_name}")
        };
        open_file_full_path.set(full_path.clone());

        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(content) =
                api::get_text(&format!("/agents/{id}/file/{full_path}")).await
            {
                file_content.set(Some(content.clone()));
                file_edit_text.set(content);
                is_editing_file.set(false);
            }
        });
    };

    let save_file = move |_| {
        let id = aid.get_value();
        let full_path = open_file_full_path.get_untracked();
        if full_path.is_empty() {
            return;
        }
        let content = file_edit_text.get_untracked();
        wasm_bindgen_futures::spawn_local(async move {
            let body = serde_json::json!({ "content": content });
            let _ = api::put::<serde_json::Value>(
                &format!("/agents/{id}/file/{full_path}"),
                &body,
            )
            .await;
            is_editing_file.set(false);
            file_content.set(Some(file_edit_text.get_untracked()));
        });
    };

    // ---------------------------------------------------------------------------
    // View
    // ---------------------------------------------------------------------------

    view! {
        <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
            {move || agent.get().map(|a| {
                match &*a {
                    Some(a) => {
                        let id = a.id.clone();
                        let name = a.name.clone();
                        let emoji = a.emoji.clone();
                        let has_soul = a.has_soul;
                        let location = a.location.clone();

                        view! {
                            // Header bar (Bug 19: no status badge)
                            <div class="flex items-center gap-2 mb-2">
                                <span class="text-lg">{emoji}</span>
                                <span class="font-bold text-lg">{name}</span>
                                // Bug 14: Chat / Details toggle
                                <div class="ml-auto join">
                                    <button
                                        class=move || if view_mode.get() == "chat" { "btn btn-sm join-item btn-active" } else { "btn btn-sm join-item" }
                                        on:click=move |_| view_mode.set("chat".to_string())
                                    >"Chat"</button>
                                    <button
                                        class=move || if view_mode.get() == "details" { "btn btn-sm join-item btn-active" } else { "btn btn-sm join-item" }
                                        on:click=move |_| view_mode.set("details".to_string())
                                    >"Details"</button>
                                </div>
                            </div>
                            <div class="divider my-1"></div>

                            // Content area
                            <div class="flex-1 flex flex-col min-h-0">
                                {
                                    let id_for_view = id.clone();
                                    let location_for_view = location.clone();
                                    move || {
                                    let mode = view_mode.get();
                                    let _id_v = id_for_view.clone();
                                    let _loc_v = location_for_view.clone();
                                    match mode.as_str() {
                                        // ============================================================
                                        // CHAT VIEW (Bug 15: fixed container + scroll)
                                        // ============================================================
                                        "chat" => {
                                            let send = send_msg;
                                            view! {
                                                // Full-height chat container
                                                <div class="flex-1 flex flex-col min-h-0 h-full">
                                                    // Scrollable messages area (Bug 15)
                                                    <div class="flex-1 overflow-y-auto min-h-0 p-2">
                                                        {move || {
                                                            let msgs = chat_messages.get();
                                                            if msgs.is_empty() {
                                                                view! {
                                                                    <div role="alert" class="alert">
                                                                        <span>"No messages yet. Send a message to start a conversation."</span>
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                msgs.into_iter().filter_map(|m| {
                                                                    let text = extract_message_text(&m);
                                                                    if text.is_empty() { return None; }
                                                                    match m.msg_type.as_str() {
                                                                        "user_task" | "user_reply" => Some(view! {
                                                                            <div class="chat chat-end mb-2">
                                                                                <div class="chat-header">"You"</div>
                                                                                <div class="chat-bubble chat-bubble-primary">{text}</div>
                                                                            </div>
                                                                        }.into_any()),
                                                                        "assistant" => {
                                                                            let html = render_markdown(&text);
                                                                            Some(view! {
                                                                                <div class="chat chat-start mb-2">
                                                                                    <div class="chat-header">"Agent"</div>
                                                                                    <div class="chat-bubble markdown-body" inner_html=html></div>
                                                                                </div>
                                                                            }.into_any())
                                                                        }
                                                                        // Bug 16: Tool messages collapsed
                                                                        "tool_result" => {
                                                                            let tool_name = m.data.get("tool")
                                                                                .and_then(|v| v.as_str())
                                                                                .unwrap_or("tool")
                                                                                .to_string();
                                                                            Some(view! {
                                                                                <div class="collapse collapse-arrow bg-base-200 mb-2">
                                                                                    <input type="checkbox" />
                                                                                    <div class="collapse-title text-sm py-1 min-h-0">
                                                                                        {format!("\u{1F527} {tool_name}")}
                                                                                    </div>
                                                                                    <div class="collapse-content">
                                                                                        <pre class="text-xs whitespace-pre-wrap max-h-[200px] overflow-y-auto bg-base-300 p-2 rounded">{text}</pre>
                                                                                    </div>
                                                                                </div>
                                                                            }.into_any())
                                                                        }
                                                                        _ => None,
                                                                    }
                                                                }).collect_view().into_any()
                                                            }
                                                        }}
                                                    </div>
                                                    // Fixed input area at bottom (Bug 17: Ctrl+Enter to send)
                                                    <div class="border-t border-base-300 p-2">
                                                        <div class="flex items-end gap-2">
                                                            <div class="flex-1 min-w-0">
                                                                <textarea class="textarea textarea-bordered w-full resize-none"
                                                                    rows="2"
                                                                    placeholder="Type a message... (Ctrl+Enter to send)"
                                                                    prop:value=move || msg_input.get()
                                                                    on:input=move |ev: ev::Event| msg_input.set(event_target_value(&ev))
                                                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                                        if ev.key() == "Enter" && ev.ctrl_key() {
                                                                            ev.prevent_default();
                                                                            send();
                                                                        }
                                                                    }
                                                                ></textarea>
                                                            </div>
                                                            <select class="select select-bordered select-sm"
                                                                prop:value=move || msg_priority.get()
                                                                on:change=move |ev: ev::Event| msg_priority.set(event_target_value(&ev))
                                                            >
                                                                <option value="collect">"Collect"</option>
                                                                <option value="steer">"Steer"</option>
                                                            </select>
                                                            <button class="btn btn-primary btn-sm"
                                                                on:click=move |_| (send_msg)()
                                                            >"Send"</button>
                                                        </div>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        }

                                        // ============================================================
                                        // DETAILS VIEW (Bug 14/20: tabs for info, files, tools, skills, logs)
                                        // ============================================================
                                        "details" => {
                                            view! {
                                                // Detail tabs
                                                <div role="tablist" class="tabs tabs-bordered mb-3">
                                                    {["info", "files", "tools", "skills", "logs"].into_iter().map(|tab| {
                                                        let label = match tab {
                                                            "info" => "Info",
                                                            "files" => "Files",
                                                            "tools" => "Tools",
                                                            "skills" => "Skills",
                                                            "logs" => "Logs",
                                                            _ => tab,
                                                        };
                                                        view! {
                                                            <a
                                                                role="tab"
                                                                class=move || if details_tab.get() == tab { "tab tab-active" } else { "tab" }
                                                                on:click=move |_| details_tab.set(tab.to_string())
                                                            >{label}</a>
                                                        }
                                                    }).collect_view()}
                                                </div>

                                                <div class="flex-1 flex flex-col min-h-0 overflow-y-auto">
                                                    {
                                                        let id_for_details = _id_v.clone();
                                                        let loc_for_details = _loc_v.clone();
                                                        move || {
                                                        let dt = details_tab.get();
                                                        let id_d = id_for_details.clone();
                                                        let loc_d = loc_for_details.clone();
                                                        match dt.as_str() {
                                                            // ----- Info -----
                                                            "info" => view! {
                                                                <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                                                                    <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                        <span class="text-sm font-bold opacity-70">"ID"</span>
                                                                        <p>{id_d}</p>
                                                                    </div></div>
                                                                    <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                        <span class="text-sm font-bold opacity-70">"Status"</span>
                                                                        <p>{move || agent_status.get()}</p>
                                                                    </div></div>
                                                                    <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                        <span class="text-sm font-bold opacity-70">"Has SOUL.md"</span>
                                                                        <p>{if has_soul { "Yes" } else { "No" }}</p>
                                                                    </div></div>
                                                                    <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                        <span class="text-sm font-bold opacity-70">"Location"</span>
                                                                        <code>{loc_d}</code>
                                                                    </div></div>
                                                                    <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                        <span class="text-sm font-bold opacity-70">"Tools"</span>
                                                                        <p>{move || tools_list.get().len().to_string()}</p>
                                                                    </div></div>
                                                                    <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                        <span class="text-sm font-bold opacity-70">"Skills"</span>
                                                                        <p>{move || skills_list.get().len().to_string()}</p>
                                                                    </div></div>
                                                                </div>
                                                            }.into_any(),

                                                            // ----- Files -----
                                                            "files" => {
                                                                let fetch_dir = fetch_dir;
                                                                view! {
                                                                    <div class="card bg-base-100 shadow-xl">
                                                                        <div class="card-body">
                                                                            <div class="flex items-center gap-1 text-sm breadcrumbs">
                                                                                <button class="btn btn-ghost btn-xs" on:click={ let fd = fetch_dir; move |_| fd(String::new()) }>"/"</button>
                                                                                {move || {
                                                                                    let path = file_path.get();
                                                                                    if path.is_empty() {
                                                                                        view! { <span></span> }.into_any()
                                                                                    } else {
                                                                                        let parts: Vec<&str> = path.split('/').collect();
                                                                                        parts.iter().enumerate().map(|(i, part)| {
                                                                                            let partial = parts[..=i].join("/");
                                                                                            let part_str = part.to_string();
                                                                                            let fd = fetch_dir;
                                                                                            view! {
                                                                                                <span class="opacity-50">" / "</span>
                                                                                                <button class="btn btn-ghost btn-xs" on:click=move |_| fd(partial.clone())>{part_str}</button>
                                                                                            }
                                                                                        }).collect_view().into_any()
                                                                                    }
                                                                                }}
                                                                            </div>
                                                                            <div class="divider my-1"></div>
                                                                            <div class="flex gap-4">
                                                                                <div class="w-[280px] shrink-0 max-h-[500px] overflow-y-auto border border-base-300 rounded-lg">
                                                                                    {move || {
                                                                                        let path = file_path.get();
                                                                                        let entries = file_entries.get();
                                                                                        let mut views = Vec::new();
                                                                                        if !path.is_empty() {
                                                                                            let parent = if let Some(pos) = path.rfind('/') { path[..pos].to_string() } else { String::new() };
                                                                                            let fd = fetch_dir;
                                                                                            views.push(view! { <button class="btn btn-ghost btn-sm w-full justify-start" on:click=move |_| fd(parent.clone())>"../"</button> }.into_any());
                                                                                        }
                                                                                        for entry in entries {
                                                                                            let name = entry.name.clone();
                                                                                            let is_dir = entry.entry_type == "directory";
                                                                                            let size_str = format_file_size(entry.size);
                                                                                            if is_dir {
                                                                                                let dir_path = if path.is_empty() { name.clone() } else { format!("{path}/{name}") };
                                                                                                let fd = fetch_dir;
                                                                                                views.push(view! { <button class="btn btn-ghost btn-sm w-full justify-start" on:click=move |_| fd(dir_path.clone())>{format!("\u{1F4C1} {name}")}</button> }.into_any());
                                                                                            } else {
                                                                                                let fname = name.clone();
                                                                                                views.push(view! {
                                                                                                    <div class="flex justify-between items-center px-2">
                                                                                                        <button class="btn btn-ghost btn-sm justify-start flex-1 min-w-0" on:click=move |_| open_file(fname.clone())>{format!("\u{1F4C4} {name}")}</button>
                                                                                                        <span class="text-xs opacity-50 shrink-0">{size_str}</span>
                                                                                                    </div>
                                                                                                }.into_any());
                                                                                            }
                                                                                        }
                                                                                        views.collect_view()
                                                                                    }}
                                                                                </div>
                                                                                <div class="flex-1 min-w-0 border border-base-300 rounded-lg p-3 overflow-auto">
                                                                                    {move || {
                                                                                        match file_content.get() {
                                                                                            Some(content) => {
                                                                                                if is_editing_file.get() {
                                                                                                    view! {
                                                                                                        <textarea class="textarea textarea-bordered w-full min-h-[300px] font-mono text-sm"
                                                                                                            prop:value=move || file_edit_text.get()
                                                                                                            on:input=move |ev: ev::Event| file_edit_text.set(event_target_value(&ev))
                                                                                                        ></textarea>
                                                                                                        <div class="flex items-center gap-2 mt-2">
                                                                                                            <button class="btn btn-primary btn-sm" on:click=save_file>"Save"</button>
                                                                                                            <button class="btn btn-ghost btn-sm" on:click=move |_| is_editing_file.set(false)>"Cancel"</button>
                                                                                                        </div>
                                                                                                    }.into_any()
                                                                                                } else {
                                                                                                    view! {
                                                                                                        <button class="btn btn-ghost btn-sm mb-2" on:click=move |_| is_editing_file.set(true)>"Edit"</button>
                                                                                                        <pre class="text-sm whitespace-pre-wrap break-all">{content}</pre>
                                                                                                    }.into_any()
                                                                                                }
                                                                                            }
                                                                                            None => view! { <div role="alert" class="alert"><span>"Select a file to view"</span></div> }.into_any(),
                                                                                        }
                                                                                    }}
                                                                                </div>
                                                                            </div>
                                                                        </div>
                                                                    </div>
                                                                }.into_any()
                                                            }

                                                            // ----- Tools -----
                                                            "tools" => {
                                                                view! {
                                                                    <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                        {move || {
                                                                            let tools = tools_list.get();
                                                                            if tools.is_empty() {
                                                                                view! { <div role="alert" class="alert"><span>"No tools loaded"</span></div> }.into_any()
                                                                            } else {
                                                                                tools.into_iter().map(|tool| {
                                                                                    let name = tool.name.clone();
                                                                                    let desc = tool.description.clone();
                                                                                    let is_enabled = !tool.disabled;
                                                                                    let name_for_toggle = name.clone();
                                                                                    view! {
                                                                                        <div class="flex justify-between items-center py-2">
                                                                                            <div>
                                                                                                <span class="font-bold">{name}</span><br />
                                                                                                <span class="text-sm opacity-70">{desc}</span>
                                                                                            </div>
                                                                                            <input type="checkbox" class="toggle toggle-primary"
                                                                                                checked=is_enabled
                                                                                                on:change=move |ev: ev::Event| {
                                                                                                    let checked = event_target_checked(&ev);
                                                                                                    toggle_tool(name_for_toggle.clone(), !checked);
                                                                                                }
                                                                                            />
                                                                                        </div>
                                                                                        <div class="divider my-0"></div>
                                                                                    }
                                                                                }).collect_view().into_any()
                                                                            }
                                                                        }}
                                                                    </div></div>
                                                                }.into_any()
                                                            }

                                                            // ----- Skills -----
                                                            "skills" => {
                                                                view! {
                                                                    <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                        {move || {
                                                                            let skills = skills_list.get();
                                                                            if skills.is_empty() {
                                                                                view! { <div role="alert" class="alert"><span>"No skills loaded"</span></div> }.into_any()
                                                                            } else {
                                                                                skills.into_iter().map(|skill| {
                                                                                    let badge_class = if skill.available { "badge badge-success" } else { "badge badge-error" };
                                                                                    let badge_text = if skill.available { "Available" } else { "Blocked" };
                                                                                    view! {
                                                                                        <div class="flex justify-between items-center py-2">
                                                                                            <div>
                                                                                                <span class="font-bold">{skill.name}</span><br />
                                                                                                <span class="text-sm opacity-70">{skill.description}</span>
                                                                                            </div>
                                                                                            <span class=badge_class>{badge_text}</span>
                                                                                        </div>
                                                                                        <div class="divider my-0"></div>
                                                                                    }
                                                                                }).collect_view().into_any()
                                                                            }
                                                                        }}
                                                                    </div></div>
                                                                }.into_any()
                                                            }

                                                            // ----- Logs (Bug 20/13) -----
                                                            "logs" => {
                                                                view! {
                                                                    <div class="card bg-base-100 shadow-xl flex-1"><div class="card-body">
                                                                        <h3 class="font-bold text-sm mb-2">"Worker Log"</h3>
                                                                        <div class="overflow-y-auto max-h-[500px] bg-base-200 rounded p-2">
                                                                            {move || {
                                                                                let lines = agent_logs.get();
                                                                                if lines.is_empty() {
                                                                                    view! { <span class="opacity-50 text-sm">"No logs yet"</span> }.into_any()
                                                                                } else {
                                                                                    view! {
                                                                                        <pre class="text-xs whitespace-pre-wrap">{lines.join("\n")}</pre>
                                                                                    }.into_any()
                                                                                }
                                                                            }}
                                                                        </div>
                                                                    </div></div>
                                                                }.into_any()
                                                            }

                                                            _ => view! { <div role="alert" class="alert alert-error"><span>"Unknown tab"</span></div> }.into_any(),
                                                        }
                                                    }}
                                                </div>
                                            }.into_any()
                                        }

                                        _ => view! {
                                            <div role="alert" class="alert alert-error">
                                                <span>"Unknown view"</span>
                                            </div>
                                        }.into_any(),
                                    }
                                }}
                            </div>
                        }.into_any()
                    }
                    None => view! {
                        <div role="alert" class="alert alert-error">
                            <span>"Agent not found"</span>
                        </div>
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_text_from_string_content() {
        let msg = SessionMsg {
            msg_id: 1,
            timestamp: String::new(),
            msg_type: "assistant".into(),
            data: serde_json::json!({"content": "hello"}),
        };
        assert_eq!(extract_message_text(&msg), "hello");
    }

    #[test]
    fn extract_text_from_array_content() {
        let msg = SessionMsg {
            msg_id: 2,
            timestamp: String::new(),
            msg_type: "assistant".into(),
            data: serde_json::json!({
                "content": [
                    {"type": "text", "text": "line1"},
                    {"type": "image", "url": "img"},
                    {"type": "text", "text": "line2"}
                ]
            }),
        };
        assert_eq!(extract_message_text(&msg), "line1\nline2");
    }

    #[test]
    fn extract_text_from_data_string() {
        let msg = SessionMsg {
            msg_id: 3,
            timestamp: String::new(),
            msg_type: "user_task".into(),
            data: serde_json::json!("hello world"),
        };
        assert_eq!(extract_message_text(&msg), "hello world");
    }

    #[test]
    fn format_file_size_bytes() {
        assert_eq!(format_file_size(500), "500 B");
    }

    #[test]
    fn format_file_size_kb() {
        assert_eq!(format_file_size(2048), "2.0 KB");
    }

    #[test]
    fn format_file_size_mb() {
        assert_eq!(format_file_size(1_500_000), "1.4 MB");
    }

    #[test]
    fn agent_summary_deserialize_backend_format() {
        let json = r#"{"id":"a1","name":"bot","emoji":"🤖","state":"running","team_id":null}"#;
        let a: AgentSummary = serde_json::from_str(json).unwrap();
        assert_eq!(a.id, "a1");
        assert_eq!(a.state, "running");
    }

    #[test]
    fn agent_detail_deserialize_backend_format() {
        let json = r#"{
            "id": "a1",
            "name": "bot",
            "emoji": "🤖",
            "state": "idle",
            "tools": ["shell"],
            "skills": [],
            "has_soul": true,
            "location": "/tmp/agent"
        }"#;
        let d: AgentDetailData = serde_json::from_str(json).unwrap();
        assert_eq!(d.id, "a1");
        assert!(d.has_soul);
        assert_eq!(d.state, "idle");
    }
}
