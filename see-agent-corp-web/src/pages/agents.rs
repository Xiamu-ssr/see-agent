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
    #[serde(default)]
    team_id: Option<String>,
    #[serde(default)]
    team_name: Option<String>,
    #[serde(default)]
    is_system: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentDetailData {
    id: String,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
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
    #[serde(default)]
    group: String,
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
    if let Some(error) = msg.data.get("error").and_then(|t| t.as_str()) {
        return error.to_string();
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

async fn copy_to_clipboard(text: String) {
    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        let _ = wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&text)).await;
    }
}

/// Look backward through messages to find tool_calls input args for a given tool_result.
fn find_tool_input(msgs: &[SessionMsg], current_idx: usize, tool_call_id: &str, tool_name: &str) -> String {
    // Search backward from current_idx for an assistant message with matching tool_calls
    for i in (0..current_idx).rev() {
        let m = &msgs[i];
        if m.msg_type != "assistant" {
            continue;
        }
        // Check tool_calls array in data
        if let Some(tool_calls) = m.data.get("tool_calls").and_then(|v| v.as_array()) {
            for tc in tool_calls {
                // Match by tool_call_id if available, otherwise by function name
                let tc_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let tc_name = tc.get("function")
                    .and_then(|v| v.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let matches = if !tool_call_id.is_empty() {
                    tc_id == tool_call_id
                } else {
                    tc_name == tool_name
                };

                if matches
                    && let Some(args) = tc.get("function")
                        .and_then(|v| v.get("arguments"))
                        .and_then(|v| v.as_str())
                {
                    // Try to pretty-print JSON args
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(args) {
                        return serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| args.to_string());
                    }
                    return args.to_string();
                }
            }
        }
        // Only search the most recent assistant message
        break;
    }
    String::new()
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
            // --- Left sidebar: agent list grouped by team ---
            <div class="w-56 shrink-0 border-r border-base-300 overflow-y-auto bg-base-200">
                <div class="p-2">
                    <h3 class="text-sm font-bold opacity-70 px-2 mb-2">"Agents"</h3>
                </div>
                <Suspense fallback=|| view! { <span class="loading loading-spinner loading-sm m-4"></span> }>
                    {move || agents.get().map(|list| {
                        // System agent at top
                        let system_agent: Option<AgentSummary> = list.iter().find(|a| a.is_system).cloned();

                        // Group non-system agents by team
                        let non_system: Vec<_> = list.iter().filter(|a| !a.is_system).cloned().collect();
                        let mut team_groups: std::collections::BTreeMap<String, (String, Vec<AgentSummary>)> = std::collections::BTreeMap::new();
                        let mut ungrouped: Vec<AgentSummary> = Vec::new();

                        for a in non_system {
                            if let (Some(tid), Some(tname)) = (a.team_id.clone(), a.team_name.clone()) {
                                team_groups.entry(tid).or_insert_with(|| (tname, Vec::new())).1.push(a);
                            } else {
                                ungrouped.push(a);
                            }
                        }

                        view! {
                            // System agent
                            {system_agent.map(|sa| {
                                let href = format!("/agents/{}", sa.id);
                                let aid = sa.id.clone();
                                view! {
                                    <A href=href attr:class=move || {
                                        let base = "block px-3 py-2 hover:bg-base-300 no-underline text-inherit transition-colors";
                                        if agent_id.get() == aid { format!("{base} bg-base-300 border-l-2 border-primary") } else { base.to_string() }
                                    }>
                                        <div class="flex items-center gap-2">
                                            <span>{sa.emoji}</span>
                                            <div class="text-sm font-bold">{sa.name}</div>
                                        </div>
                                    </A>
                                    <div class="divider my-0 h-0"></div>
                                }
                            })}
                            // Team groups
                            {team_groups.into_iter().map(|(_tid, (tname, members))| {
                                view! {
                                    <div class="px-3 py-1">
                                        <div class="text-xs font-bold opacity-50 uppercase tracking-wide">{tname}</div>
                                    </div>
                                    {members.into_iter().map(|a| {
                                        let href = format!("/agents/{}", a.id);
                                        let aid = a.id.clone();
                                        let badge_class = status_badge_class(&a.state);
                                        view! {
                                            <A href=href attr:class=move || {
                                                let base = "block px-3 py-2 pl-5 hover:bg-base-300 no-underline text-inherit transition-colors";
                                                if agent_id.get() == aid { format!("{base} bg-base-300 border-l-2 border-primary") } else { base.to_string() }
                                            }>
                                                <div class="flex items-center gap-2">
                                                    <span>{a.emoji}</span>
                                                    <div class="flex-1 min-w-0">
                                                        <div class="text-sm font-bold truncate">{a.name}</div>
                                                        <span class=format!("{badge_class} badge-xs")>{a.state.clone()}</span>
                                                    </div>
                                                </div>
                                            </A>
                                        }
                                    }).collect_view()}
                                    <div class="divider my-0 h-0"></div>
                                }
                            }).collect_view()}
                            // Ungrouped agents
                            {if !ungrouped.is_empty() {
                                Some(view! {
                                    <div class="px-3 py-1">
                                        <div class="text-xs font-bold opacity-50 uppercase tracking-wide">"No Team"</div>
                                    </div>
                                    {ungrouped.into_iter().map(|a| {
                                        let href = format!("/agents/{}", a.id);
                                        let aid = a.id.clone();
                                        let badge_class = status_badge_class(&a.state);
                                        view! {
                                            <A href=href attr:class=move || {
                                                let base = "block px-3 py-2 pl-5 hover:bg-base-300 no-underline text-inherit transition-colors";
                                                if agent_id.get() == aid { format!("{base} bg-base-300 border-l-2 border-primary") } else { base.to_string() }
                                            }>
                                                <div class="flex items-center gap-2">
                                                    <span>{a.emoji}</span>
                                                    <div class="flex-1 min-w-0">
                                                        <div class="text-sm font-bold truncate">{a.name}</div>
                                                        <span class=format!("{badge_class} badge-xs")>{a.state.clone()}</span>
                                                    </div>
                                                </div>
                                            </A>
                                        }
                                    }).collect_view()}
                                })
                            } else {
                                None
                            }}
                        }
                    })}
                </Suspense>
            </div>

            // --- Right panel: agent detail ---
            <div class="flex-1 flex flex-col min-h-0 overflow-hidden p-4">
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
    let chat_container_ref = NodeRef::<leptos::html::Div>::new();
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

    // --- Fetch per-agent skills ---
    {
        let id = agent_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(skills) = api::get::<Vec<SkillInfo>>(&format!("/agents/{id}/skills")).await {
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

    // --- Sticky scroll: auto-scroll only when near bottom ---
    let is_near_bottom = RwSignal::new(true);
    // Track scroll position to determine if user is near bottom
    let on_chat_scroll = move |_: ev::Event| {
        if let Some(el) = chat_container_ref.get() {
            let el: web_sys::Element = el.into();
            let distance = el.scroll_height() - el.scroll_top() - el.client_height();
            is_near_bottom.set(distance < 100);
        }
    };
    // On new messages, only scroll if near bottom (sticky behavior)
    Effect::new(move |_| {
        let _msgs = chat_messages.get();
        if is_near_bottom.get_untracked()
            && let Some(el) = chat_container_ref.get()
        {
            let el: web_sys::Element = el.into();
            request_animation_frame(move || {
                el.set_scroll_top(el.scroll_height());
            });
        }
    });
    // Scroll-to-bottom handler for floating button
    let scroll_to_bottom = move |_: ev::MouseEvent| {
        if let Some(el) = chat_container_ref.get() {
            let el: web_sys::Element = el.into();
            el.set_scroll_top(el.scroll_height());
        }
        is_near_bottom.set(true);
    };

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

    // --- Skill toggle handler ---
    let toggle_skill = move |name: String, new_disabled: bool| {
        let id = aid.get_value();
        let body = serde_json::json!({ "disabled": new_disabled });
        skills_list.update(|list| {
            if let Some(skill) = list.iter_mut().find(|s| s.name == name) {
                skill.disabled = new_disabled;
                skill.available = !new_disabled;
            }
        });
        wasm_bindgen_futures::spawn_local(async move {
            let _ = api::post::<serde_json::Value>(
                &format!("/agents/{id}/skills/{name}/toggle"),
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

    let agent_id_outer = agent_id.clone();
    view! {
        <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
            {let agent_id_inner = agent_id_outer.clone(); move || agent.get().map(|a| {
                let agent_id = agent_id_inner.clone();
                match &*a {
                    Some(a) => {
                        let id = a.id.clone();
                        let has_soul = a.has_soul;
                        let location = a.location.clone();

                        view! {
                            // Header bar: Chat/Details toggle only (Bug 59: removed emoji+name)
                            <div class="flex items-center gap-2 mb-2">
                                <div class="join">
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
                                                <div class="flex-1 flex flex-col min-h-0 h-full relative">
                                                    // Scrollable messages area
                                                    <div node_ref=chat_container_ref class="flex-1 overflow-y-auto min-h-0 p-2"
                                                        on:scroll=on_chat_scroll>
                                                        {move || {
                                                            let msgs: Vec<SessionMsg> = chat_messages.get().into_iter().collect();
                                                            if msgs.is_empty() {
                                                                view! {
                                                                    <div role="alert" class="alert">
                                                                        <span>"还没有消息。发送消息开始对话。"</span>
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                let msgs_clone = msgs.clone();
                                                                msgs.into_iter().enumerate().filter_map(move |(idx, m)| {
                                                                    let text = extract_message_text(&m);
                                                                    if text.is_empty() { return None; }
                                                                    // Strip [xxx] prefix from content if present
                                                                    let display_text = if text.starts_with('[') {
                                                                        if let Some(end) = text.find("] ") {
                                                                            text[end + 2..].to_string()
                                                                        } else { text.clone() }
                                                                    } else { text.clone() };
                                                                    let _ = idx;
                                                                    match m.msg_type.as_str() {
                                                                        "user_task" | "user_reply" => {
                                                                            let sender = m.data.get("sender")
                                                                                .and_then(|v| v.as_str())
                                                                                .unwrap_or("user");
                                                                            let header = match sender {
                                                                                "user" => "You".to_string(),
                                                                                "system" | "supervisor" => "System".to_string(),
                                                                                other => other.to_string(),
                                                                            };
                                                                            let is_steer = m.data.get("priority")
                                                                                .and_then(|v| v.as_str())
                                                                                == Some("steer");
                                                                            let steer_badge = if is_steer {
                                                                                " \u{26A1} steer"
                                                                            } else { "" };
                                                                            Some(view! {
                                                                                <div class="chat chat-end mb-2">
                                                                                    <div class="chat-header">{header}{steer_badge}</div>
                                                                                    <div class="chat-bubble chat-bubble-primary">{display_text.clone()}</div>
                                                                                </div>
                                                                            }.into_any())
                                                                        }
                                                                        "assistant" => {
                                                                            let html = render_markdown(&text);
                                                                            Some(view! {
                                                                                <div class="chat chat-start mb-2">
                                                                                    <div class="chat-header">"Agent"</div>
                                                                                    <div class="chat-bubble markdown-body" inner_html=html></div>
                                                                                </div>
                                                                            }.into_any())
                                                                        }
                                                                        // Bug 16/48: Tool messages collapsed with input params
                                                                        "tool_result" => {
                                                                            let tool_name = m.data.get("tool")
                                                                                .and_then(|v| v.as_str())
                                                                                .unwrap_or("tool")
                                                                                .to_string();
                                                                            let tool_call_id = m.data.get("tool_call_id")
                                                                                .and_then(|v| v.as_str())
                                                                                .unwrap_or("")
                                                                                .to_string();
                                                                            // Look back for matching tool_calls in preceding assistant messages
                                                                            let input_args = find_tool_input(&msgs_clone, idx, &tool_call_id, &tool_name);
                                                                            Some(view! {
                                                                                <div class="collapse collapse-arrow bg-base-200 mb-2">
                                                                                    <input type="checkbox" />
                                                                                    <div class="collapse-title text-sm py-1 min-h-0">
                                                                                        {format!("\u{1F527} {tool_name}")}
                                                                                    </div>
                                                                                    <div class="collapse-content">
                                                                                        {if !input_args.is_empty() {
                                                                                            Some(view! {
                                                                                                <div class="text-xs mb-1 opacity-70">"Input:"</div>
                                                                                                <pre class="text-xs whitespace-pre-wrap max-h-[120px] overflow-y-auto bg-base-300 p-2 rounded mb-2">{input_args}</pre>
                                                                                            })
                                                                                        } else { None }}
                                                                                        <div class="text-xs mb-1 opacity-70">"Result:"</div>
                                                                                        <pre class="text-xs whitespace-pre-wrap max-h-[200px] overflow-y-auto bg-base-300 p-2 rounded">{text}</pre>
                                                                                    </div>
                                                                                </div>
                                                                            }.into_any())
                                                                        }
                                                                        "error" => {
                                                                            let error_text = m.data.get("error")
                                                                                .and_then(|v| v.as_str())
                                                                                .unwrap_or(&text)
                                                                                .to_string();
                                                                            Some(view! {
                                                                                <div class="alert alert-error text-sm mb-2">
                                                                                    <span>{format!("\u{26A0}\u{FE0F} {error_text}")}</span>
                                                                                </div>
                                                                            }.into_any())
                                                                        }
                                                                        _ => None,
                                                                    }
                                                                }).collect_view().into_any()
                                                            }
                                                        }}
                                                    </div>
                                                    // Floating scroll-to-bottom button
                                                    {move || if !is_near_bottom.get() {
                                                        Some(view! {
                                                            <button class="btn btn-circle btn-sm btn-ghost absolute bottom-16 left-1/2 -translate-x-1/2 opacity-70 z-10 bg-base-200"
                                                                on:click=scroll_to_bottom
                                                            >"\u{2193}"</button>
                                                        })
                                                    } else { None }}
                                                    // Fixed input area at bottom
                                                    <div class="border-t border-base-300 p-2">
                                                        <div class="flex items-end gap-1">
                                                            <textarea class="textarea textarea-bordered textarea-sm flex-1 resize-none min-h-[36px] max-h-[120px]"
                                                                rows="1"
                                                                placeholder="消息... (Ctrl+Enter 发送)"
                                                                prop:value=move || msg_input.get()
                                                                on:input=move |ev: ev::Event| msg_input.set(event_target_value(&ev))
                                                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                                    if ev.key() == "Enter" && ev.ctrl_key() {
                                                                        ev.prevent_default();
                                                                        send();
                                                                    }
                                                                }
                                                            ></textarea>
                                                            <select class="select select-bordered select-xs w-20"
                                                                prop:value=move || msg_priority.get()
                                                                on:change=move |ev: ev::Event| msg_priority.set(event_target_value(&ev))
                                                            >
                                                                <option value="collect">"普通"</option>
                                                                <option value="steer">"加急"</option>
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
                                                        let agent_id_for_files = agent_id.clone();
                                                        move || {
                                                        let dt = details_tab.get();
                                                        let id_d = id_for_details.clone();
                                                        let loc_d = loc_for_details.clone();
                                                        let agent_id_files = agent_id_for_files.clone();
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
                                                                                                let copy_path = if path.is_empty() {
                                                                                                    format!("~/.see-agent-corp/agents/{}/{name}", agent_id_files)
                                                                                                } else {
                                                                                                    format!("~/.see-agent-corp/agents/{}/{path}/{name}", agent_id_files)
                                                                                                };
                                                                                                views.push(view! {
                                                                                                    <div class="flex justify-between items-center px-2">
                                                                                                        <button class="btn btn-ghost btn-sm justify-start flex-1 min-w-0" on:click=move |_| open_file(fname.clone())>{format!("\u{1F4C4} {name}")}</button>
                                                                                                        <button class="btn btn-ghost btn-xs px-1 opacity-50 hover:opacity-100" title="Copy path" on:click={
                                                                                                            let cp = copy_path.clone();
                                                                                                            move |_| {
                                                                                                                let cp2 = cp.clone();
                                                                                                                wasm_bindgen_futures::spawn_local(copy_to_clipboard(cp2));
                                                                                                            }
                                                                                                        }>"\u{1F4CB}"</button>
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
                                                                                                        <pre class="text-sm whitespace-pre-wrap break-all max-h-[70vh] overflow-y-auto">{content}</pre>
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

                                                            // ----- Tools (grouped) -----
                                                            "tools" => {
                                                                view! {
                                                                    <div class="card bg-base-100 shadow-xl"><div class="card-body">
                                                                        {move || {
                                                                            let tools = tools_list.get();
                                                                            if tools.is_empty() {
                                                                                view! { <div role="alert" class="alert"><span>"No tools loaded"</span></div> }.into_any()
                                                                            } else {
                                                                                // Group tools by group name
                                                                                let mut groups: std::collections::BTreeMap<String, Vec<ToolInfo>> = std::collections::BTreeMap::new();
                                                                                for tool in tools {
                                                                                    let g = if tool.group.is_empty() { "other".to_string() } else { tool.group.clone() };
                                                                                    groups.entry(g).or_default().push(tool);
                                                                                }
                                                                                groups.into_iter().map(|(group_name, group_tools)| {
                                                                                    let count = group_tools.len();
                                                                                    let title = format!("{group_name} ({count})");
                                                                                    view! {
                                                                                        <div class="collapse collapse-arrow bg-base-200 mb-2">
                                                                                            <input type="checkbox" checked=true />
                                                                                            <div class="collapse-title font-medium text-sm capitalize">{title}</div>
                                                                                            <div class="collapse-content">
                                                                                                {group_tools.into_iter().map(|tool| {
                                                                                                    let name = tool.name.clone();
                                                                                                    let desc = tool.description.clone();
                                                                                                    let is_enabled = !tool.disabled;
                                                                                                    let name_for_toggle = name.clone();
                                                                                                    view! {
                                                                                                        <div class="flex justify-between items-center py-1">
                                                                                                            <div>
                                                                                                                <span class="font-bold text-sm">{name}</span>
                                                                                                                <span class="text-xs opacity-70 ml-2">{desc}</span>
                                                                                                            </div>
                                                                                                            <input type="checkbox" class="toggle toggle-primary toggle-sm"
                                                                                                                checked=is_enabled
                                                                                                                on:change=move |ev: ev::Event| {
                                                                                                                    let checked = event_target_checked(&ev);
                                                                                                                    toggle_tool(name_for_toggle.clone(), !checked);
                                                                                                                }
                                                                                                            />
                                                                                                        </div>
                                                                                                    }
                                                                                                }).collect_view()}
                                                                                            </div>
                                                                                        </div>
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
                                                                                    let name = skill.name.clone();
                                                                                    let desc = skill.description.clone();
                                                                                    let is_enabled = skill.available;
                                                                                    let name_for_toggle = name.clone();
                                                                                    view! {
                                                                                        <div class="flex justify-between items-center py-1">
                                                                                            <div>
                                                                                                <span class="font-bold text-sm">{name}</span>
                                                                                                <span class="text-xs opacity-70 ml-2">{desc}</span>
                                                                                            </div>
                                                                                            <input type="checkbox" class="toggle toggle-primary toggle-sm"
                                                                                                checked=is_enabled
                                                                                                on:change=move |ev: ev::Event| {
                                                                                                    let checked = event_target_checked(&ev);
                                                                                                    toggle_skill(name_for_toggle.clone(), !checked);
                                                                                                }
                                                                                            />
                                                                                        </div>
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
