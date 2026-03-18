use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct LogEntry {
    time: String,
    level: String,
    message: String,
}

#[component]
pub fn Logs() -> impl IntoView {
    let logs = LocalResource::new(|| async {
        api::get::<Vec<LogEntry>>("/logs").await.unwrap_or_default()
    });

    view! {
        <div>
            <h2 class="text-xl font-bold mb-4">"Logs"</h2>
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || logs.get().map(|list| {
                    if list.is_empty() {
                        view! {
                            <div class="text-center py-12 opacity-60">
                                <p class="text-4xl mb-2">"📋"</p>
                                <p>"No log entries"</p>
                            </div>
                        }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <div class="overflow-x-auto">
                                <table class="table">
                                    <thead>
                                        <tr>
                                            <th>"Time"</th>
                                            <th>"Level"</th>
                                            <th>"Message"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {items.into_iter().map(|e| {
                                            let badge_class = match e.level.to_lowercase().as_str() {
                                                "error" => "badge badge-error",
                                                "warn" => "badge badge-warning",
                                                "info" => "badge badge-primary",
                                                _ => "badge badge-info",
                                            };
                                            view! {
                                                <tr>
                                                    <td><span class="text-sm opacity-70">{e.time}</span></td>
                                                    <td><span class=badge_class>{e.level}</span></td>
                                                    <td><code>{e.message}</code></td>
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
