use leptos::prelude::*;
use serde::Deserialize;
use thaw::*;

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
            <Body1><b>"MCP Servers"</b></Body1>
            <Divider />
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || servers.get().map(|list| {
                    if list.is_empty() {
                        view! { <MessageBar><MessageBarBody>"No MCP servers configured."</MessageBarBody></MessageBar> }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <Grid cols=3 x_gap=12 y_gap=12>
                                {items.into_iter().map(|s| {
                                    let name = s.name;
                                    let status = s.status;
                                    let tools_label = format!("{} tools", s.tools_count);
                                    view! {
                                        <GridItem>
                                            <Card>
                                                <Caption1Strong>{name}</Caption1Strong>
                                                <Badge color=BadgeColor::Informative>{status}</Badge>
                                                <Caption1>{tools_label}</Caption1>
                                            </Card>
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
