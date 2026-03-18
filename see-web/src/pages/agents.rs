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
    status: String,
    #[allow(dead_code)]
    team_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentDetailData {
    id: String,
    name: String,
    emoji: String,
    status: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Chat,
    Files,
    Tools,
    Skills,
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

// ---------------------------------------------------------------------------
// List page
// ---------------------------------------------------------------------------

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

#[component]
pub fn AgentDetail() -> impl IntoView {
    let params = use_params_map();
    let (tab, set_tab) = signal(Tab::Overview);
    let (msg_input, set_msg_input) = signal(String::new());
    let (msg_priority, set_msg_priority) = signal("collect".to_string());
    let chat_messages = RwSignal::new(Vec::<SessionMsg>::new());
    let tools_list = RwSignal::new(Vec::<ToolInfo>::new());
    let skills_list = RwSignal::new(Vec::<SkillInfo>::new());
    let file_entries = RwSignal::new(Vec::<FileEntry>::new());
    let file_path = RwSignal::new(String::new());
    let file_content = RwSignal::new(Option::<String>::None);
    let file_edit_text = RwSignal::new(String::new());
    let is_editing_file = RwSignal::new(false);
    let agent_status = RwSignal::new("idle".to_string());
    let is_active = RwSignal::new(true);

    let agent_id = Memo::new(move |_| {
        params.read().get("id").unwrap_or_default()
    });

    let agent = LocalResource::new(move || {
        let id = agent_id.get();
        async move {
            if id.is_empty() {
                None
            } else {
                let result = api::get::<AgentDetailData>(&format!("/agents/{id}")).await.ok();
                if let Some(ref data) = result {
                    agent_status.set(data.status.clone());
                }
                result
            }
        }
    });

    // --- Polling for chat messages ---
    on_cleanup(move || is_active.set(false));
    {
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(2000).await;
                if !is_active.get_untracked() {
                    break;
                }
                let id = agent_id.get_untracked();
                if id.is_empty() {
                    continue;
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

    // --- Fetch root files ---
    {
        wasm_bindgen_futures::spawn_local(async move {
            // Wait for agent_id to be populated
            gloo_timers::future::TimeoutFuture::new(100).await;
            let id = agent_id.get_untracked();
            if !id.is_empty()
                && let Ok(entries) =
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
        let id = agent_id.get();
        let priority = msg_priority.get();
        set_msg_input.set(String::new());

        wasm_bindgen_futures::spawn_local(async move {
            let body = serde_json::json!({
                "content": content,
                "priority": priority
            });
            let _ =
                api::post::<serde_json::Value>(&format!("/agents/{id}/message"), &body).await;
        });
    };

    let start_agent = move |_| {
        let id = agent_id.get();
        agent_status.set("running".to_string());
        wasm_bindgen_futures::spawn_local(async move {
            let _ = api::post::<serde_json::Value>(
                &format!("/agents/{id}/start"),
                &serde_json::json!({}),
            )
            .await;
        });
    };

    let stop_agent = move |_| {
        let id = agent_id.get();
        agent_status.set("idle".to_string());
        wasm_bindgen_futures::spawn_local(async move {
            let _ = api::post::<serde_json::Value>(
                &format!("/agents/{id}/stop"),
                &serde_json::json!({}),
            )
            .await;
        });
    };

    // --- Tool toggle handler ---
    let toggle_tool = move |name: String, new_disabled: bool| {
        let body = serde_json::json!({ "disabled": new_disabled });
        tools_list.update(|list| {
            if let Some(tool) = list.iter_mut().find(|t| t.name == name) {
                tool.disabled = new_disabled;
            }
        });
        wasm_bindgen_futures::spawn_local(async move {
            let _ =
                api::post::<serde_json::Value>(&format!("/tools/{name}/toggle"), &body).await;
        });
    };

    // --- File handlers ---
    let fetch_dir = move |path: String| {
        let id = agent_id.get_untracked();
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
        let id = agent_id.get_untracked();
        let current = file_path.get_untracked();
        let full_path = if current.is_empty() {
            file_name
        } else {
            format!("{current}/{file_name}")
        };

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
        let id = agent_id.get_untracked();
        let current = file_path.get_untracked();
        let content = file_edit_text.get_untracked();

        // Determine file path from the open file
        // The file_content being Some means a file is open
        let file_name = current.clone(); // Use file_path as the directory context
        wasm_bindgen_futures::spawn_local(async move {
            let body = serde_json::json!({ "content": content });
            let _ = api::put::<serde_json::Value>(
                &format!("/agents/{id}/file/{file_name}"),
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
        <div class="page">
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || agent.get().map(|a| {
                    match &*a {
                        Some(a) => {
                            let id = a.id.clone();
                            let name = a.name.clone();
                            let emoji = a.emoji.clone();
                            let has_soul = a.has_soul;
                            let location = a.location.clone();

                            view! {
                                <div class="detail-header">
                                    <A href="/agents" attr:class="back-link">"< Agents"</A>
                                    <span class="detail-emoji">{emoji}</span>
                                    <h2>{name}</h2>
                                    <span class=move || format!("status-badge status-{}", agent_status.get())>
                                        {move || agent_status.get()}
                                    </span>
                                    <div class="agent-actions">
                                        <button class="btn btn-sm" on:click=start_agent>"Start"</button>
                                        <button class="btn btn-sm" on:click=stop_agent>"Stop"</button>
                                    </div>
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
                                                    <span class="info-value">{move || agent_status.get()}</span>
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
                                                    <span class="info-value">{move || tools_list.get().len()}</span>
                                                </div>
                                                <div class="info-card">
                                                    <span class="info-label">"Skills"</span>
                                                    <span class="info-value">{move || skills_list.get().len()}</span>
                                                </div>
                                            </div>
                                        }.into_any(),

                                        // -------------------------------------------------------
                                        // Chat tab
                                        // -------------------------------------------------------
                                        Tab::Chat => {
                                            let send = send_msg;
                                            view! {
                                                <div class="chat-panel">
                                                    <div class="chat-log">
                                                        {move || {
                                                            let msgs = chat_messages.get();
                                                            if msgs.is_empty() {
                                                                view! {
                                                                    <p class="empty chat-empty">"No messages yet. Send a message to start a conversation."</p>
                                                                }.into_any()
                                                            } else {
                                                                msgs.into_iter().filter_map(|m| {
                                                                    let text = extract_message_text(&m);
                                                                    if text.is_empty() { return None; }
                                                                    match m.msg_type.as_str() {
                                                                        "user_task" | "user_reply" => Some(view! {
                                                                            <div class="chat-msg chat-you">
                                                                                <span class="chat-sender">"You"</span>
                                                                                <span class="chat-content">{text}</span>
                                                                            </div>
                                                                        }.into_any()),
                                                                        "assistant" => {
                                                                            let html = render_markdown(&text);
                                                                            Some(view! {
                                                                                <div class="chat-msg chat-agent">
                                                                                    <span class="chat-sender">"Agent"</span>
                                                                                    <div class="chat-content markdown-body" inner_html=html></div>
                                                                                </div>
                                                                            }.into_any())
                                                                        }
                                                                        "tool_result" => Some(view! {
                                                                            <div class="chat-msg chat-tool">
                                                                                <span class="chat-sender">"Tool"</span>
                                                                                <pre class="chat-content tool-output">{text}</pre>
                                                                            </div>
                                                                        }.into_any()),
                                                                        _ => None,
                                                                    }
                                                                }).collect_view().into_any()
                                                            }
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
                                                        <select
                                                            class="priority-select"
                                                            on:change=move |ev| {
                                                                set_msg_priority.set(event_target_value(&ev));
                                                            }
                                                        >
                                                            <option value="collect">"Collect"</option>
                                                            <option value="steer">"Steer"</option>
                                                        </select>
                                                        <button
                                                            class="btn btn-primary"
                                                            on:click=move |_| (send_msg)()
                                                        >"Send"</button>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        }

                                        // -------------------------------------------------------
                                        // Files tab
                                        // -------------------------------------------------------
                                        Tab::Files => {
                                            let fetch_dir = fetch_dir;
                                            view! {
                                                <div class="file-browser">
                                                    <div class="file-breadcrumb">
                                                        <button
                                                            class="breadcrumb-btn"
                                                            on:click={
                                                                let fd = fetch_dir;
                                                                move |_| fd(String::new())
                                                            }
                                                        >"/"</button>
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
                                                                        <span class="breadcrumb-sep">" / "</span>
                                                                        <button
                                                                            class="breadcrumb-btn"
                                                                            on:click=move |_| fd(partial.clone())
                                                                        >{part_str}</button>
                                                                    }
                                                                }).collect_view().into_any()
                                                            }
                                                        }}
                                                    </div>

                                                    <div class="file-browser-layout">
                                                        <div class="file-list">
                                                            {move || {
                                                                let path = file_path.get();
                                                                let entries = file_entries.get();
                                                                let mut views = Vec::new();

                                                                // ".." entry if not at root
                                                                if !path.is_empty() {
                                                                    let parent = if let Some(pos) = path.rfind('/') {
                                                                        path[..pos].to_string()
                                                                    } else {
                                                                        String::new()
                                                                    };
                                                                    let fd = fetch_dir;
                                                                    views.push(view! {
                                                                        <div
                                                                            class="file-entry file-dir"
                                                                            on:click=move |_| fd(parent.clone())
                                                                        >
                                                                            <span class="file-icon">"../"</span>
                                                                            <span class="file-name">".."</span>
                                                                        </div>
                                                                    }.into_any());
                                                                }

                                                                for entry in entries {
                                                                    let name = entry.name.clone();
                                                                    let is_dir = entry.entry_type == "directory";
                                                                    let size_str = format_file_size(entry.size);

                                                                    if is_dir {
                                                                        let dir_path = if path.is_empty() {
                                                                            name.clone()
                                                                        } else {
                                                                            format!("{path}/{name}")
                                                                        };
                                                                        let fd = fetch_dir;
                                                                        views.push(view! {
                                                                            <div
                                                                                class="file-entry file-dir"
                                                                                on:click=move |_| fd(dir_path.clone())
                                                                            >
                                                                                <span class="file-icon">"📁"</span>
                                                                                <span class="file-name">{name}</span>
                                                                            </div>
                                                                        }.into_any());
                                                                    } else {
                                                                        let fname = name.clone();
                                                                        views.push(view! {
                                                                            <div
                                                                                class="file-entry file-file"
                                                                                on:click=move |_| open_file(fname.clone())
                                                                            >
                                                                                <span class="file-icon">"📄"</span>
                                                                                <span class="file-name">{name}</span>
                                                                                <span class="file-size">{size_str.clone()}</span>
                                                                            </div>
                                                                        }.into_any());
                                                                    }
                                                                }

                                                                views.collect_view()
                                                            }}
                                                        </div>

                                                        <div class="file-content-panel">
                                                            {move || {
                                                                match file_content.get() {
                                                                    Some(content) => {
                                                                        if is_editing_file.get() {
                                                                            view! {
                                                                                <div class="file-editor">
                                                                                    <textarea
                                                                                        class="file-textarea"
                                                                                        prop:value=file_edit_text
                                                                                        on:input=move |ev| {
                                                                                            file_edit_text.set(event_target_value(&ev));
                                                                                        }
                                                                                    />
                                                                                    <div class="file-editor-actions">
                                                                                        <button class="btn btn-primary btn-sm" on:click=save_file>"Save"</button>
                                                                                        <button class="btn btn-sm" on:click=move |_| is_editing_file.set(false)>"Cancel"</button>
                                                                                    </div>
                                                                                </div>
                                                                            }.into_any()
                                                                        } else {
                                                                            view! {
                                                                                <div class="file-viewer">
                                                                                    <button class="btn btn-sm" on:click=move |_| is_editing_file.set(true)>"Edit"</button>
                                                                                    <pre class="file-pre">{content}</pre>
                                                                                </div>
                                                                            }.into_any()
                                                                        }
                                                                    }
                                                                    None => view! {
                                                                        <p class="empty">"Select a file to view"</p>
                                                                    }.into_any(),
                                                                }
                                                            }}
                                                        </div>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        }

                                        // -------------------------------------------------------
                                        // Tools tab
                                        // -------------------------------------------------------
                                        Tab::Tools => {
                                            view! {
                                                <div class="tools-panel">
                                                    {move || {
                                                        let tools = tools_list.get();
                                                        if tools.is_empty() {
                                                            view! { <p class="empty">"No tools loaded"</p> }.into_any()
                                                        } else {
                                                            tools.into_iter().map(|tool| {
                                                                let name = tool.name.clone();
                                                                let desc = tool.description.clone();
                                                                let disabled = tool.disabled;
                                                                let name_for_toggle = name.clone();
                                                                view! {
                                                                    <div class="tool-item">
                                                                        <div class="tool-info">
                                                                            <span class="tool-name">{name}</span>
                                                                            <span class="tool-desc">{desc}</span>
                                                                        </div>
                                                                        <label class="toggle">
                                                                            <input
                                                                                type="checkbox"
                                                                                prop:checked=!disabled
                                                                                on:change=move |_| {
                                                                                    toggle_tool(name_for_toggle.clone(), !disabled);
                                                                                }
                                                                            />
                                                                            <span class="toggle-slider"></span>
                                                                        </label>
                                                                    </div>
                                                                }
                                                            }).collect_view().into_any()
                                                        }
                                                    }}
                                                </div>
                                            }.into_any()
                                        }

                                        // -------------------------------------------------------
                                        // Skills tab
                                        // -------------------------------------------------------
                                        Tab::Skills => {
                                            view! {
                                                <div class="skills-panel">
                                                    {move || {
                                                        let skills = skills_list.get();
                                                        if skills.is_empty() {
                                                            view! { <p class="empty">"No skills loaded"</p> }.into_any()
                                                        } else {
                                                            skills.into_iter().map(|skill| {
                                                                let status_class = if skill.available {
                                                                    "skill-status skill-available"
                                                                } else {
                                                                    "skill-status skill-blocked"
                                                                };
                                                                let status_text = if skill.available { "Available" } else { "Blocked" };
                                                                view! {
                                                                    <div class="skill-item">
                                                                        <div class="skill-info">
                                                                            <span class="skill-name">{skill.name}</span>
                                                                            <span class="skill-desc">{skill.description}</span>
                                                                        </div>
                                                                        <span class=status_class>{status_text}</span>
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
                            <p class="error">"Agent not found"</p>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
