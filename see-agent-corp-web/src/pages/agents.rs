use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde::Deserialize;
use thaw::*;

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

fn status_badge_color(status: &str) -> BadgeColor {
    match status {
        "running" => BadgeColor::Success,
        "idle" => BadgeColor::Subtle,
        "error" => BadgeColor::Danger,
        _ => BadgeColor::Informative,
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
        <div class="page-content">
            <span class="page-header">"Agents"</span>
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || agents.get().map(|list| {
                    let items: Vec<_> = list.iter().cloned().collect();
                    if items.is_empty() {
                        view! {
                            <div class="empty-state">
                                <div class="empty-state-icon">"🤖"</div>
                                <div class="empty-state-text">"No agents yet"</div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <Grid cols=3 x_gap=12 y_gap=12>
                                {items.into_iter().map(|a| {
                                    let href = format!("/agents/{}", a.id);
                                    let badge_color = status_badge_color(&a.state);
                                    let status_text = a.state;
                                    view! {
                                        <GridItem>
                                            <A href=href attr:style="text-decoration:none;color:inherit">
                                                <Card class="card-interactive">
                                                    <span style="font-size:1.5rem">{a.emoji}</span>
                                                    <Caption1Strong>{a.name}</Caption1Strong>
                                                    <Badge color=badge_color>{status_text}</Badge>
                                                </Card>
                                            </A>
                                        </GridItem>
                                    }
                                }).collect_view()}
                            </Grid>
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
    let selected_tab = RwSignal::new(String::from("overview"));
    let msg_input = RwSignal::new(String::new());
    let msg_priority = RwSignal::new(String::from("collect"));
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
                    agent_status.set(data.state.clone());
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
        let id = agent_id.get_untracked();
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
        let file_name = current.clone();
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
        <div style="flex:1;display:flex;flex-direction:column;min-height:0">
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || agent.get().map(|a| {
                    match &*a {
                        Some(a) => {
                            let id = a.id.clone();
                            let name = a.name.clone();
                            let emoji = a.emoji.clone();
                            let has_soul = a.has_soul;
                            let location = a.location.clone();

                            view! {
                                <Flex vertical=false align=FlexAlign::Center gap=FlexGap::Small>
                                    <A href="/agents">
                                        <Button appearance=ButtonAppearance::Subtle>"< Agents"</Button>
                                    </A>
                                    <Caption1>{emoji}</Caption1>
                                    <Body1><b>{name}</b></Body1>
                                    {move || {
                                        let s = agent_status.get();
                                        let color = status_badge_color(&s);
                                        view! { <Badge color=color>{s}</Badge> }
                                    }}
                                    <Button appearance=ButtonAppearance::Primary on_click=start_agent>"Start"</Button>
                                    <Button appearance=ButtonAppearance::Subtle on_click=stop_agent>"Stop"</Button>
                                </Flex>
                                <Divider />

                                <TabList selected_value=selected_tab>
                                    <Tab value="overview">"Overview"</Tab>
                                    <Tab value="chat">"Chat"</Tab>
                                    <Tab value="files">"Files"</Tab>
                                    <Tab value="tools">"Tools"</Tab>
                                    <Tab value="skills">"Skills"</Tab>
                                </TabList>

                                <div style="margin-top:12px;flex:1;display:flex;flex-direction:column;min-height:0">
                                    {
                                        let id_for_tab = id.clone();
                                        let location_for_tab = location.clone();
                                        move || {
                                        let current_tab = selected_tab.get();
                                        let id_display = id_for_tab.clone();
                                        let loc_display = location_for_tab.clone();
                                        match current_tab.as_str() {
                                            // ----- Overview -----
                                            "overview" => view! {
                                                <Grid cols=3 x_gap=12 y_gap=12>
                                                    <GridItem>
                                                        <Card>
                                                            <Caption1Strong>"ID"</Caption1Strong>
                                                            <Body1>{id_display}</Body1>
                                                        </Card>
                                                    </GridItem>
                                                    <GridItem>
                                                        <Card>
                                                            <Caption1Strong>"Status"</Caption1Strong>
                                                            <Body1>{move || agent_status.get()}</Body1>
                                                        </Card>
                                                    </GridItem>
                                                    <GridItem>
                                                        <Card>
                                                            <Caption1Strong>"Has SOUL.md"</Caption1Strong>
                                                            <Body1>{if has_soul { "Yes" } else { "No" }}</Body1>
                                                        </Card>
                                                    </GridItem>
                                                    <GridItem>
                                                        <Card>
                                                            <Caption1Strong>"Location"</Caption1Strong>
                                                            <code>{loc_display}</code>
                                                        </Card>
                                                    </GridItem>
                                                    <GridItem>
                                                        <Card>
                                                            <Caption1Strong>"Tools"</Caption1Strong>
                                                            <Body1>{move || tools_list.get().len().to_string()}</Body1>
                                                        </Card>
                                                    </GridItem>
                                                    <GridItem>
                                                        <Card>
                                                            <Caption1Strong>"Skills"</Caption1Strong>
                                                            <Body1>{move || skills_list.get().len().to_string()}</Body1>
                                                        </Card>
                                                    </GridItem>
                                                </Grid>
                                            }.into_any(),

                                            // ----- Chat -----
                                            "chat" => {
                                                let send = send_msg;
                                                view! {
                                                    <Card attr:style="flex:1;display:flex;flex-direction:column;min-height:0">
                                                        <div style="flex:1;overflow-y:auto;padding:8px;min-height:200px">
                                                            {move || {
                                                                let msgs = chat_messages.get();
                                                                if msgs.is_empty() {
                                                                    view! {
                                                                        <MessageBar><MessageBarBody>"No messages yet. Send a message to start a conversation."</MessageBarBody></MessageBar>
                                                                    }.into_any()
                                                                } else {
                                                                    msgs.into_iter().filter_map(|m| {
                                                                        let text = extract_message_text(&m);
                                                                        if text.is_empty() { return None; }
                                                                        match m.msg_type.as_str() {
                                                                            "user_task" | "user_reply" => Some(view! {
                                                                                <div style="text-align:right;margin-bottom:8px">
                                                                                    <Tag>"You"</Tag>
                                                                                    <div style="background:var(--colorBrandBackground);color:var(--colorNeutralForegroundOnBrand);padding:8px;border-radius:8px;display:inline-block;max-width:80%;text-align:left">
                                                                                        {text}
                                                                                    </div>
                                                                                </div>
                                                                            }.into_any()),
                                                                            "assistant" => {
                                                                                let html = render_markdown(&text);
                                                                                Some(view! {
                                                                                    <div style="text-align:left;margin-bottom:8px">
                                                                                        <Tag>"Agent"</Tag>
                                                                                        <div style="padding:8px" class="markdown-body" inner_html=html></div>
                                                                                    </div>
                                                                                }.into_any())
                                                                            }
                                                                            "tool_result" => Some(view! {
                                                                                <div style="margin-bottom:8px">
                                                                                    <Tag>"Tool"</Tag>
                                                                                    <pre style="font-size:0.8rem;white-space:pre-wrap;max-height:200px;overflow-y:auto">{text}</pre>
                                                                                </div>
                                                                            }.into_any()),
                                                                            _ => None,
                                                                        }
                                                                    }).collect_view().into_any()
                                                                }
                                                            }}
                                                        </div>
                                                        <Divider />
                                                        <Flex vertical=false gap=FlexGap::Small align=FlexAlign::Center>
                                                            <div style="flex:1;min-width:0" on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                                if ev.key() == "Enter" {
                                                                    send();
                                                                }
                                                            }>
                                                                <Input
                                                                    value=msg_input
                                                                    placeholder="Send a message..."
                                                                />
                                                            </div>
                                                            <Select value=msg_priority>
                                                                <option value="collect">"Collect"</option>
                                                                <option value="steer">"Steer"</option>
                                                            </Select>
                                                            <Button
                                                                appearance=ButtonAppearance::Primary
                                                                on_click=move |_| (send_msg)()
                                                            >"Send"</Button>
                                                        </Flex>
                                                    </Card>
                                                }.into_any()
                                            }

                                            // ----- Files -----
                                            "files" => {
                                                let fetch_dir = fetch_dir;
                                                view! {
                                                    <Card>
                                                        <Flex vertical=false gap=FlexGap::Small align=FlexAlign::Center>
                                                            <Button
                                                                appearance=ButtonAppearance::Subtle
                                                                on_click={
                                                                    let fd = fetch_dir;
                                                                    move |_| fd(String::new())
                                                                }
                                                            >"/"</Button>
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
                                                                            <Caption1>" / "</Caption1>
                                                                            <Button
                                                                                appearance=ButtonAppearance::Subtle
                                                                                on_click=move |_| fd(partial.clone())
                                                                            >{part_str}</Button>
                                                                        }
                                                                    }).collect_view().into_any()
                                                                }
                                                            }}
                                                        </Flex>
                                                        <Divider />
                                                        <Flex vertical=false gap=FlexGap::Medium>
                                                            <div style="flex:0 0 280px;max-height:500px;overflow-y:auto;border:1px solid var(--colorNeutralStroke1);border-radius:4px">
                                                                {move || {
                                                                    let path = file_path.get();
                                                                    let entries = file_entries.get();
                                                                    let mut views = Vec::new();

                                                                    if !path.is_empty() {
                                                                        let parent = if let Some(pos) = path.rfind('/') {
                                                                            path[..pos].to_string()
                                                                        } else {
                                                                            String::new()
                                                                        };
                                                                        let fd = fetch_dir;
                                                                        views.push(view! {
                                                                            <Button
                                                                                appearance=ButtonAppearance::Transparent
                                                                                on_click=move |_| fd(parent.clone())
                                                                            >"../"</Button>
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
                                                                                <Button
                                                                                    appearance=ButtonAppearance::Transparent
                                                                                    on_click=move |_| fd(dir_path.clone())
                                                                                >{format!("📁 {name}")}</Button>
                                                                            }.into_any());
                                                                        } else {
                                                                            let fname = name.clone();
                                                                            views.push(view! {
                                                                                <Flex vertical=false justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                                                                    <Button
                                                                                        appearance=ButtonAppearance::Transparent
                                                                                        on_click=move |_| open_file(fname.clone())
                                                                                    >{format!("📄 {name}")}</Button>
                                                                                    <Caption1>{size_str.clone()}</Caption1>
                                                                                </Flex>
                                                                            }.into_any());
                                                                        }
                                                                    }

                                                                    views.collect_view()
                                                                }}
                                                            </div>

                                                            <div style="flex:1;min-width:0;border:1px solid var(--colorNeutralStroke1);border-radius:4px;padding:8px;overflow:auto">
                                                                {move || {
                                                                    match file_content.get() {
                                                                        Some(content) => {
                                                                            if is_editing_file.get() {
                                                                                view! {
                                                                                    <Textarea
                                                                                        value=file_edit_text
                                                                                        attr:style="width:100%;min-height:300px;font-family:monospace;font-size:0.85rem"
                                                                                    />
                                                                                    <Flex vertical=false gap=FlexGap::Small>
                                                                                        <Button appearance=ButtonAppearance::Primary on_click=save_file>"Save"</Button>
                                                                                        <Button appearance=ButtonAppearance::Subtle on_click=move |_| is_editing_file.set(false)>"Cancel"</Button>
                                                                                    </Flex>
                                                                                }.into_any()
                                                                            } else {
                                                                                view! {
                                                                                    <Button appearance=ButtonAppearance::Subtle on_click=move |_| is_editing_file.set(true)>"Edit"</Button>
                                                                                    <pre style="font-size:0.85rem;white-space:pre-wrap;word-break:break-all">{content}</pre>
                                                                                }.into_any()
                                                                            }
                                                                        }
                                                                        None => view! {
                                                                            <MessageBar><MessageBarBody>"Select a file to view"</MessageBarBody></MessageBar>
                                                                        }.into_any(),
                                                                    }
                                                                }}
                                                            </div>
                                                        </Flex>
                                                    </Card>
                                                }.into_any()
                                            }

                                            // ----- Tools -----
                                            "tools" => {
                                                view! {
                                                    <Card>
                                                        {move || {
                                                            let tools = tools_list.get();
                                                            if tools.is_empty() {
                                                                view! { <MessageBar><MessageBarBody>"No tools loaded"</MessageBarBody></MessageBar> }.into_any()
                                                            } else {
                                                                tools.into_iter().map(|tool| {
                                                                    let name = tool.name.clone();
                                                                    let desc = tool.description.clone();
                                                                    let disabled = tool.disabled;
                                                                    let name_for_toggle = name.clone();
                                                                    let checked = RwSignal::new(!disabled);
                                                                    Effect::new(move |_| {
                                                                        let is_checked = checked.get();
                                                                        let should_disable = !is_checked;
                                                                        if should_disable != disabled {
                                                                            toggle_tool(name_for_toggle.clone(), should_disable);
                                                                        }
                                                                    });
                                                                    view! {
                                                                        <Flex vertical=false justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                                                            <div>
                                                                                <Body1><b>{name}</b></Body1>
                                                                                <br />
                                                                                <Caption1>{desc}</Caption1>
                                                                            </div>
                                                                            <Switch checked=checked />
                                                                        </Flex>
                                                                        <Divider />
                                                                    }
                                                                }).collect_view().into_any()
                                                            }
                                                        }}
                                                    </Card>
                                                }.into_any()
                                            }

                                            // ----- Skills -----
                                            "skills" => {
                                                view! {
                                                    <Card>
                                                        {move || {
                                                            let skills = skills_list.get();
                                                            if skills.is_empty() {
                                                                view! { <MessageBar><MessageBarBody>"No skills loaded"</MessageBarBody></MessageBar> }.into_any()
                                                            } else {
                                                                skills.into_iter().map(|skill| {
                                                                    let badge_color = if skill.available { BadgeColor::Success } else { BadgeColor::Danger };
                                                                    let badge_text = if skill.available { "Available" } else { "Blocked" };
                                                                    view! {
                                                                        <Flex vertical=false justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                                                            <div>
                                                                                <Body1><b>{skill.name}</b></Body1>
                                                                                <br />
                                                                                <Caption1>{skill.description}</Caption1>
                                                                            </div>
                                                                            <Badge color=badge_color>{badge_text}</Badge>
                                                                        </Flex>
                                                                        <Divider />
                                                                    }
                                                                }).collect_view().into_any()
                                                            }
                                                        }}
                                                    </Card>
                                                }.into_any()
                                            }

                                            _ => view! {
                                                <MessageBar><MessageBarBody>"Unknown tab"</MessageBarBody></MessageBar>
                                            }.into_any(),
                                        }
                                    }}
                                </div>
                            }.into_any()
                        }
                        None => view! {
                            <MessageBar><MessageBarBody>"Agent not found"</MessageBarBody></MessageBar>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
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
        // Backend sends "state" not "status"
        let json = r#"{"id":"a1","name":"bot","emoji":"🤖","state":"running","team_id":null}"#;
        let a: AgentSummary = serde_json::from_str(json).unwrap();
        assert_eq!(a.id, "a1");
        assert_eq!(a.state, "running");
    }

    #[test]
    fn agent_detail_deserialize_backend_format() {
        // Backend sends "state" not "status"
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
