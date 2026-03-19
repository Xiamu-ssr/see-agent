use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct ToolInfo {
    name: String,
    description: String,
    #[serde(default)]
    group: String,
}

#[component]
pub fn Tools() -> impl IntoView {
    let tools = LocalResource::new(|| async {
        api::get::<Vec<ToolInfo>>("/tools").await.unwrap_or_default()
    });

    view! {
        <div class="h-full overflow-y-auto">
            <h2 class="text-xl font-bold mb-4">"Tools"</h2>
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || tools.get().map(|list| {
                    if list.is_empty() {
                        view! {
                            <div class="text-center py-12 opacity-60">
                                <p class="text-4xl mb-2">"🔧"</p>
                                <p>"No tools registered"</p>
                            </div>
                        }.into_any()
                    } else {
                        // Group tools by group name
                        let mut groups: std::collections::BTreeMap<String, Vec<ToolInfo>> = std::collections::BTreeMap::new();
                        for tool in list.iter().cloned() {
                            let g = if tool.group.is_empty() { "other".to_string() } else { tool.group.clone() };
                            groups.entry(g).or_default().push(tool);
                        }
                        view! {
                            <div class="max-w-3xl">
                                {groups.into_iter().map(|(group_name, group_tools)| {
                                    let count = group_tools.len();
                                    let title = format!("{group_name} ({count})");
                                    view! {
                                        <div class="collapse collapse-arrow bg-base-200 mb-2">
                                            <input type="checkbox" checked=true />
                                            <div class="collapse-title font-medium text-sm capitalize">{title}</div>
                                            <div class="collapse-content">
                                                {group_tools.into_iter().map(|tool| {
                                                    view! {
                                                        <div class="flex justify-between items-center py-1">
                                                            <div>
                                                                <span class="font-bold text-sm">{tool.name}</span>
                                                                <span class="text-xs opacity-70 ml-2">{tool.description}</span>
                                                            </div>
                                                        </div>
                                                    }
                                                }).collect_view()}
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
