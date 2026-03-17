use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct McpServer {
    name: String,
    status: String,
    tools: Vec<String>,
}

#[component]
pub fn Mcp() -> impl IntoView {
    let servers = LocalResource::new(|| async {
        api::get::<Vec<McpServer>>("/mcp/servers").await.unwrap_or_default()
    });

    view! {
        <div class="page">
            <h2>"MCP Servers"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || servers.get().map(|list| {
                    if list.is_empty() {
                        view! { <p>"No MCP servers configured"</p> }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <div class="card-grid">
                                {items.into_iter().map(|s| {
                                    let tool_count = s.tools.len();
                                    view! {
                                        <div class="card">
                                            <span class="card-name">{s.name}</span>
                                            <span class="card-status">{s.status}</span>
                                            <span class="card-meta">{format!("{tool_count} tools")}</span>
                                        </div>
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
