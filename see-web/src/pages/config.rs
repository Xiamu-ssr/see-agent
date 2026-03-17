use leptos::prelude::*;
use serde_json::Value;

use crate::api;

#[component]
pub fn Config() -> impl IntoView {
    let config = LocalResource::new(|| async {
        api::get::<Value>("/config").await.ok()
    });

    let (editing, set_editing) = signal(false);
    let (edit_text, set_edit_text) = signal(String::new());
    let (save_error, set_save_error) = signal::<Option<String>>(None);

    let start_edit = move |val: &Value| {
        let pretty = serde_json::to_string_pretty(val).unwrap_or_default();
        set_edit_text.set(pretty);
        set_editing.set(true);
        set_save_error.set(None);
    };

    let save = move || {
        let text = edit_text.get();
        match serde_json::from_str::<Value>(&text) {
            Ok(val) => {
                set_save_error.set(None);
                wasm_bindgen_futures::spawn_local(async move {
                    match api::put::<Value>("/config", &val).await {
                        Ok(_) => {
                            set_editing.set(false);
                            // Page will re-fetch on next render
                        }
                        Err(e) => set_save_error.set(Some(e)),
                    }
                });
            }
            Err(e) => set_save_error.set(Some(format!("Invalid JSON: {e}"))),
        }
    };

    view! {
        <div class="page">
            <div class="page-header">
                <h2>"Configuration"</h2>
            </div>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || config.get().map(|c| {
                    match &*c {
                        Some(val) => {
                            let pretty = serde_json::to_string_pretty(val).unwrap_or_default();
                            let val_clone = val.clone();
                            view! {
                                {move || {
                                    if editing.get() {
                                        view! {
                                            <div class="config-editor">
                                                <textarea
                                                    class="config-textarea"
                                                    prop:value=edit_text
                                                    on:input=move |ev| {
                                                        set_edit_text.set(event_target_value(&ev));
                                                    }
                                                />
                                                {move || save_error.get().map(|e| view! {
                                                    <p class="error">{e}</p>
                                                })}
                                                <div class="config-actions">
                                                    <button class="btn btn-primary" on:click=move |_| save()>"Save"</button>
                                                    <button class="btn" on:click=move |_| set_editing.set(false)>"Cancel"</button>
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else {
                                        let vc = val_clone.clone();
                                        view! {
                                            <button class="btn" on:click=move |_| start_edit(&vc)>"Edit"</button>
                                            <pre class="config-json">{pretty.clone()}</pre>
                                        }.into_any()
                                    }
                                }}
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
