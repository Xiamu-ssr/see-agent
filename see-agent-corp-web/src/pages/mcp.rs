use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct McpServer {
    name: String,
    status: String,
    tools_count: usize,
}

#[component]
pub fn Mcp() -> impl IntoView {
    let servers = LocalResource::new(|| async {
        api::get::<Vec<McpServer>>("/mcp/servers").await.unwrap_or_default()
    });

    view! {
        <div>
            <h2 class="text-xl font-bold mb-4">"MCP Servers"</h2>
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || servers.get().map(|list| {
                    if list.is_empty() {
                        view! {
                            <div class="text-center py-12 opacity-60">
                                <p class="text-4xl mb-2">"🔌"</p>
                                <p>"No MCP servers configured"</p>
                            </div>
                        }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                                {items.into_iter().map(|s| {
                                    let tools_label = format!("{} tools", s.tools_count);
                                    view! {
                                        <div class="card bg-base-100 shadow-xl">
                                            <div class="card-body">
                                                <h3 class="card-title text-sm">{s.name}</h3>
                                                <span class="badge badge-info">{s.status}</span>
                                                <span class="text-sm opacity-70">{tools_label}</span>
                                            </div>
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
