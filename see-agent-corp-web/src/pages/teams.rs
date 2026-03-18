use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde::Deserialize;
use thaw::*;

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
            <Body1><b>"Teams"</b></Body1>
            <Divider />
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || teams.get().map(|list| {
                    let items: Vec<_> = list.iter().cloned().collect();
                    if items.is_empty() {
                        view! { <MessageBar><MessageBarBody>"No teams yet"</MessageBarBody></MessageBar> }.into_any()
                    } else {
                        view! {
                            <Grid cols=3 x_gap=12 y_gap=12>
                                {items.into_iter().map(|t| {
                                    let href = format!("/teams/{}", t.id);
                                    let count = t.members.len();
                                    let name = t.name;
                                    let status = t.status;
                                    view! {
                                        <GridItem>
                                            <A href=href attr:style="text-decoration:none;color:inherit">
                                                <Card>
                                                    <Caption1Strong>{name}</Caption1Strong>
                                                    <Badge color=BadgeColor::Informative>{status}</Badge>
                                                    <Caption1>{format!("{count} members")}</Caption1>
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
pub fn TeamDetail() -> impl IntoView {
    let params = use_params_map();
    let selected_tab = RwSignal::new(String::from("members"));
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
        <div>
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || team.get().map(|t| {
                    match &*t {
                        Some(t) => {
                            let members = t.members.clone();
                            let team_name = t.name.clone();
                            let team_status = t.status.clone();

                            view! {
                                <Flex vertical=false align=FlexAlign::Center gap=FlexGap::Small>
                                    <A href="/teams">
                                        <Button appearance=ButtonAppearance::Subtle>"< Teams"</Button>
                                    </A>
                                    <Body1><b>{team_name}</b></Body1>
                                    <Badge color=BadgeColor::Informative>{team_status}</Badge>
                                </Flex>
                                <Divider />

                                <TabList selected_value=selected_tab>
                                    <Tab value="members">"Members"</Tab>
                                    <Tab value="tasks">"Task Board"</Tab>
                                    <Tab value="messages">"Messages"</Tab>
                                </TabList>

                                <div style="margin-top:12px">
                                    {move || {
                                        let current_tab = selected_tab.get();
                                        match current_tab.as_str() {
                                            // ----- Members -----
                                            "members" => {
                                                let members = members.clone();
                                                view! {
                                                    <Table>
                                                        <TableHeader>
                                                            <TableRow>
                                                                <TableHeaderCell>"Agent ID"</TableHeaderCell>
                                                                <TableHeaderCell>"Role"</TableHeaderCell>
                                                            </TableRow>
                                                        </TableHeader>
                                                        <TableBody>
                                                            {members.into_iter().map(|m| {
                                                                let agent_href = format!("/agents/{}", m.id);
                                                                let mid = m.id;
                                                                let role = m.role;
                                                                view! {
                                                                    <TableRow>
                                                                        <TableCell><TableCellLayout><A href=agent_href>{mid}</A></TableCellLayout></TableCell>
                                                                        <TableCell><TableCellLayout>{role}</TableCellLayout></TableCell>
                                                                    </TableRow>
                                                                }
                                                            }).collect_view()}
                                                        </TableBody>
                                                    </Table>
                                                }.into_any()
                                            }

                                            // ----- Task Board -----
                                            "tasks" => {
                                                view! {
                                                    <Card>
                                                        <Flex vertical=false gap=FlexGap::Small align=FlexAlign::Center>
                                                            <Input value=new_task_title placeholder="Task title..." />
                                                            <Input value=new_task_desc placeholder="Description (optional)" />
                                                            <Button appearance=ButtonAppearance::Primary on_click=create_task>"Create Task"</Button>
                                                        </Flex>
                                                    </Card>
                                                    <Grid cols=4 x_gap=8 y_gap=8>
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
                                                                <GridItem>
                                                                    <Card>
                                                                        <Caption1Strong>{status_label}</Caption1Strong>
                                                                        <Divider />
                                                                        {move || {
                                                                            let all_tasks = tasks.get();
                                                                            let filtered: Vec<_> = all_tasks.into_iter()
                                                                                .filter(|t| t.status == status_owned)
                                                                                .collect();

                                                                            if filtered.is_empty() {
                                                                                view! { <Caption1>"—"</Caption1> }.into_any()
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
                                                                                        <Card>
                                                                                            <Body1><b>{task.title}</b></Body1>
                                                                                            {if !task.description.is_empty() {
                                                                                                Some(view! { <Caption1>{task.description}</Caption1> })
                                                                                            } else { None }}
                                                                                            <Flex vertical=false gap=FlexGap::Small>
                                                                                                {task.assigned_to.map(|a| view! {
                                                                                                    <Badge color=BadgeColor::Brand>{a}</Badge>
                                                                                                })}
                                                                                                <Caption1>{format!("by {}", task.created_by)}</Caption1>
                                                                                            </Flex>
                                                                                            {task.result.map(|r| view! {
                                                                                                <MessageBar><MessageBarBody>{r}</MessageBarBody></MessageBar>
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
                                                                                                    <Button
                                                                                                        appearance=ButtonAppearance::Primary
                                                                                                        on_click=move |_| update_task_status(tid.clone(), ns_str.clone())
                                                                                                    >{label}</Button>
                                                                                                }
                                                                                            })}
                                                                                        </Card>
                                                                                    }
                                                                                }).collect_view().into_any()
                                                                            }
                                                                        }}
                                                                    </Card>
                                                                </GridItem>
                                                            }
                                                        }).collect_view()}
                                                    </Grid>
                                                }.into_any()
                                            }

                                            // ----- Messages -----
                                            "messages" => {
                                                view! {
                                                    <div>
                                                        {move || {
                                                            let msgs = messages.get();
                                                            if msgs.is_empty() {
                                                                view! { <MessageBar><MessageBarBody>"No messages yet"</MessageBarBody></MessageBar> }.into_any()
                                                            } else {
                                                                msgs.into_iter().map(|m| {
                                                                    let priority_color = match m.priority.as_str() {
                                                                        "steer" => BadgeColor::Warning,
                                                                        _ => BadgeColor::Brand,
                                                                    };
                                                                    let sender = m.sender;
                                                                    let priority = m.priority;
                                                                    let timestamp = m.timestamp;
                                                                    let content = m.content;
                                                                    view! {
                                                                        <Card>
                                                                            <Flex vertical=false justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                                                                <Body1><b>{sender}</b></Body1>
                                                                                <Flex vertical=false gap=FlexGap::Small align=FlexAlign::Center>
                                                                                    <Badge color=priority_color>{priority}</Badge>
                                                                                    <Caption1>{timestamp}</Caption1>
                                                                                </Flex>
                                                                            </Flex>
                                                                            <Body1>{content}</Body1>
                                                                        </Card>
                                                                    }
                                                                }).collect_view().into_any()
                                                            }
                                                        }}
                                                    </div>
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
                            <MessageBar><MessageBarBody>"Team not found"</MessageBarBody></MessageBar>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
