use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
}

#[component]
pub fn Logs() -> impl IntoView {
    let logs = LocalResource::new(|| async {
        api::get::<Vec<LogEntry>>("/logs").await.unwrap_or_default()
    });

    view! {
        <div class="page">
            <h2>"Logs"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || logs.get().map(|list| {
                    if list.is_empty() {
                        view! { <p>"No log entries"</p> }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <div class="log-list">
                                {items.into_iter().map(|e| {
                                    let level_class = format!("log-{}", e.level.to_lowercase());
                                    view! {
                                        <div class="log-entry">
                                            <span class="log-time">{e.timestamp}</span>
                                            <span class=level_class>{e.level}</span>
                                            <span class="log-msg">{e.message}</span>
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
