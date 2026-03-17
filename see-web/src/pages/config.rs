use leptos::prelude::*;
use serde_json::Value;

use crate::api;

#[component]
pub fn Config() -> impl IntoView {
    let config = LocalResource::new(|| async {
        api::get::<Value>("/config").await.ok()
    });

    view! {
        <div class="page">
            <h2>"Configuration"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || config.get().map(|c| {
                    match &*c {
                        Some(val) => {
                            let pretty = serde_json::to_string_pretty(&val).unwrap_or_default();
                            view! {
                                <pre class="config-json">{pretty}</pre>
                            }.into_any()
                        }
                        None => view! {
                            <p class="error">"Could not load config"</p>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
