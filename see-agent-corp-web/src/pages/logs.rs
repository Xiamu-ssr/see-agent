use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct LogEntry {
    #[serde(default)]
    time: String,
    #[serde(default)]
    level: String,
    #[serde(default)]
    source: String,
    message: String,
}

#[component]
pub fn Logs() -> impl IntoView {
    let logs = LocalResource::new(|| async {
        api::get::<Vec<LogEntry>>("/logs").await.unwrap_or_default()
    });

    view! {
        <div class="h-full overflow-y-auto">
            <h2 class="text-xl font-bold mb-4">"Logs"</h2>
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || logs.get().map(|list| {
                    let items: Vec<LogEntry> = (*list).clone();
                    if items.is_empty() {
                        view! {
                            <div class="text-center py-12 opacity-60">
                                <p class="text-4xl mb-2">"📋"</p>
                                <p>"No log entries"</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="overflow-x-auto">
                                <table class="table table-sm">
                                    <thead>
                                        <tr>
                                            <th>"Time"</th>
                                            <th>"Source"</th>
                                            <th>"Level"</th>
                                            <th>"Message"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {items.into_iter().map(|e| {
                                            let level_lower = e.level.to_lowercase();
                                            let (badge_class, row_class) = match level_lower.as_str() {
                                                "error" => ("badge badge-error badge-sm", "log-error"),
                                                "warn" => ("badge badge-warning badge-sm", "log-warn"),
                                                "info" => ("badge badge-primary badge-sm", "log-info"),
                                                "debug" => ("badge badge-ghost badge-sm", "log-debug"),
                                                _ => ("badge badge-sm", ""),
                                            };
                                            view! {
                                                <tr class=row_class>
                                                    <td><span class="text-xs opacity-70 whitespace-nowrap">{e.time}</span></td>
                                                    <td><span class="text-xs font-mono">{e.source}</span></td>
                                                    <td><span class=badge_class>{e.level}</span></td>
                                                    <td><code class="text-xs">{e.message}</code></td>
                                                </tr>
                                            }
                                        }).collect_view()}
                                    </tbody>
                                </table>
                            </div>
                        }.into_any()
                    }
                })}
            </Suspense>
        </div>
    }
}
