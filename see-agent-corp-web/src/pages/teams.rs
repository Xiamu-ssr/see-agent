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
        <div class="page-content">
            <span class="page-header">"Teams"</span>
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || teams.get().map(|list| {
                    let items: Vec<_> = list.iter().cloned().collect();
                    if items.is_empty() {
                        view! {
                            <div class="empty-state">
                                <div class="empty-state-icon">"👥"</div>
                                <div class="empty-state-text">"No teams yet"</div>
                            </div>
                        }.into_any()
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
                                                <Card class="card-interactive">
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
// Detail page — single-page view (Members + Tasks + Messages all visible)
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

                                // --- Members section ---
                                <Card>
                                    <Caption1Strong>"Members"</Caption1Strong>
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
                                </Card>

                                // --- Task Board section ---
                                <Card>
                                    <Caption1Strong>"Task Board"</Caption1Strong>
                                    <Flex vertical=false gap=FlexGap::Small align=FlexAlign::Center>
                                        <Input value=new_task_title placeholder="Task title..." />
                                        <Input value=new_task_desc placeholder="Description (optional)" />
                                        <Button appearance=ButtonAppearance::Primary on_click=create_task>"Create"</Button>
                                    </Flex>
                                    <Divider />
                                    {move || {
                                        let all_tasks = tasks.get();
                                        if all_tasks.is_empty() {
                                            view! { <Caption1>"No tasks yet"</Caption1> }.into_any()
                                        } else {
                                            view! {
                                                <Table>
                                                    <TableHeader>
                                                        <TableRow>
                                                            <TableHeaderCell>"Title"</TableHeaderCell>
                                                            <TableHeaderCell>"Status"</TableHeaderCell>
                                                            <TableHeaderCell>"Assignee"</TableHeaderCell>
                                                            <TableHeaderCell>"Action"</TableHeaderCell>
                                                        </TableRow>
                                                    </TableHeader>
                                                    <TableBody>
                                                        {all_tasks.into_iter().map(|task| {
                                                            let task_id = task.id.clone();
                                                            let status_color = match task.status.as_str() {
                                                                "done" => BadgeColor::Success,
                                                                "in_progress" => BadgeColor::Brand,
                                                                "claimed" => BadgeColor::Informative,
                                                                _ => BadgeColor::Subtle,
                                                            };
                                                            let next_status = match task.status.as_str() {
                                                                "pending" => Some(("claimed", "Claim")),
                                                                "claimed" => Some(("in_progress", "Start")),
                                                                "in_progress" => Some(("done", "Complete")),
                                                                _ => None,
                                                            };
                                                            let assignee = task.assigned_to.unwrap_or_else(|| "—".into());
                                                            let status_label = task.status;
                                                            view! {
                                                                <TableRow>
                                                                    <TableCell><TableCellLayout>
                                                                        <Body1>{task.title}</Body1>
                                                                    </TableCellLayout></TableCell>
                                                                    <TableCell><TableCellLayout>
                                                                        <Badge color=status_color>{status_label}</Badge>
                                                                    </TableCellLayout></TableCell>
                                                                    <TableCell><TableCellLayout>{assignee}</TableCellLayout></TableCell>
                                                                    <TableCell><TableCellLayout>
                                                                        {next_status.map(|(ns, label)| {
                                                                            let tid = task_id.clone();
                                                                            let ns_str = ns.to_string();
                                                                            view! {
                                                                                <Button
                                                                                    size=ButtonSize::Small
                                                                                    appearance=ButtonAppearance::Primary
                                                                                    on_click=move |_| update_task_status(tid.clone(), ns_str.clone())
                                                                                >{label}</Button>
                                                                            }
                                                                        })}
                                                                    </TableCellLayout></TableCell>
                                                                </TableRow>
                                                            }
                                                        }).collect_view()}
                                                    </TableBody>
                                                </Table>
                                            }.into_any()
                                        }
                                    }}
                                </Card>

                                // --- Messages section ---
                                <Card>
                                    <Caption1Strong>"Messages"</Caption1Strong>
                                    {move || {
                                        let msgs = messages.get();
                                        if msgs.is_empty() {
                                            view! { <Caption1>"No messages yet"</Caption1> }.into_any()
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
                                                    <div style="padding:8px 0;border-bottom:1px solid var(--colorNeutralStroke2)">
                                                        <Flex vertical=false justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                                                            <Body1><b>{sender}</b></Body1>
                                                            <Flex vertical=false gap=FlexGap::Small align=FlexAlign::Center>
                                                                <Badge color=priority_color>{priority}</Badge>
                                                                <Caption1>{timestamp}</Caption1>
                                                            </Flex>
                                                        </Flex>
                                                        <Body1>{content}</Body1>
                                                    </div>
                                                }
                                            }).collect_view().into_any()
                                        }
                                    }}
                                </Card>
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
