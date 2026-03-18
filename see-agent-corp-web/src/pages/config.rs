use leptos::ev;
use leptos::prelude::*;
use serde_json::Value;

use crate::api;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_ref<'a>(schema: &'a Value, definitions: &'a Value) -> &'a Value {
    if let Some(ref_path) = schema.get("$ref").and_then(|r| r.as_str()) {
        let name = ref_path.strip_prefix("#/definitions/").unwrap_or(ref_path);
        return definitions.get(name).unwrap_or(schema);
    }
    if let Some(all_of) = schema.get("allOf").and_then(|a| a.as_array()) {
        for item in all_of {
            if let Some(ref_path) = item.get("$ref").and_then(|r| r.as_str()) {
                let name = ref_path.strip_prefix("#/definitions/").unwrap_or(ref_path);
                if let Some(resolved) = definitions.get(name) {
                    return resolved;
                }
            }
        }
    }
    schema
}

fn format_section_name(name: &str) -> String {
    name.chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                c.to_uppercase().to_string()
            } else if c == '_' {
                " ".to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn schema_type(schema: &Value) -> &str {
    schema
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("string")
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[component]
pub fn Config() -> impl IntoView {
    let schema_res = LocalResource::new(|| async {
        api::get::<Value>("/config/schema").await.ok()
    });

    let config_res = LocalResource::new(|| async {
        api::get::<Value>("/config").await.ok()
    });

    let defaults_res = LocalResource::new(|| async {
        api::get::<Value>("/config/defaults").await.ok()
    });

    let form_data = RwSignal::new(Value::Object(Default::default()));
    let save_msg = RwSignal::new(Option::<String>::None);
    let initialized = RwSignal::new(false);

    view! {
        <div>
            <h2 class="text-xl font-bold mb-4">"Configuration"</h2>
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || {
                    let schema = schema_res.get();
                    let config = config_res.get();
                    let defaults = defaults_res.get();

                    let (schema, config, defaults) = match (schema, config, defaults) {
                        (Some(s), Some(c), Some(d)) => {
                            match ((*s).clone(), (*c).clone(), (*d).clone()) {
                                (Some(s), Some(c), Some(d)) => (s, c, d),
                                _ => return view! {
                                    <div role="alert" class="alert alert-error">
                                        <span>"Could not load configuration"</span>
                                    </div>
                                }.into_any(),
                            }
                        }
                        _ => return view! { <span class="loading loading-spinner loading-lg"></span> }.into_any(),
                    };

                    {
                        if !initialized.get_untracked() {
                            form_data.set(config.clone());
                            initialized.set(true);
                        }

                        let definitions = schema.get("definitions").cloned().unwrap_or(Value::Object(Default::default()));
                        let properties = schema.get("properties").and_then(|p| p.as_object()).cloned().unwrap_or_default();
                        let sections: Vec<_> = properties.into_iter().collect();

                        view! {
                            <div class="max-w-3xl">
                                {sections.into_iter().map(|(section_name, section_schema)| {
                                    let resolved = resolve_ref(&section_schema, &definitions).clone();
                                    let section_props = resolved.get("properties").and_then(|p| p.as_object()).cloned().unwrap_or_default();
                                    let section_label = format_section_name(&section_name);
                                    let defs = definitions.clone();
                                    let section_defaults = defaults.get(&section_name).cloned().unwrap_or_default();

                                    view! {
                                        <div class="card bg-base-100 shadow-xl mb-4">
                                            <div class="card-body">
                                                <h3 class="card-title text-sm">{section_label}</h3>
                                                <div class="divider my-1"></div>
                                                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                                                    {section_props.into_iter().map(|(field_name, field_schema)| {
                                                        let resolved_field = resolve_ref(&field_schema, &defs).clone();
                                                        let field_type = schema_type(&resolved_field).to_string();
                                                        let label = format_section_name(&field_name);
                                                        let description = resolved_field.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                                                        let default_val = section_defaults.get(&field_name).cloned().unwrap_or_default();
                                                        let sn = section_name.clone();
                                                        let fn_ = field_name.clone();

                                                        let current_val = move || {
                                                            let data = form_data.get();
                                                            data.get(&sn).and_then(|s| s.get(&fn_)).cloned().unwrap_or_default()
                                                        };

                                                        let sn2 = section_name.clone();
                                                        let fn2 = field_name.clone();
                                                        let on_change = move |new_val: Value| {
                                                            let sn = sn2.clone();
                                                            let fn_ = fn2.clone();
                                                            form_data.update(|data| {
                                                                if let Some(obj) = data.as_object_mut() {
                                                                    let section = obj.entry(sn).or_insert(Value::Object(Default::default()));
                                                                    if let Some(section_map) = section.as_object_mut() {
                                                                        section_map.insert(fn_, new_val);
                                                                    }
                                                                }
                                                            });
                                                        };

                                                        let placeholder = match &default_val {
                                                            Value::String(s) => s.clone(),
                                                            Value::Number(n) => n.to_string(),
                                                            Value::Bool(b) => b.to_string(),
                                                            _ => String::new(),
                                                        };

                                                        match field_type.as_str() {
                                                            "boolean" => {
                                                                let on_change = on_change.clone();
                                                                let current_val_a = current_val.clone();
                                                                let current_val_b = current_val.clone();
                                                                let checked = RwSignal::new(current_val_a().as_bool().unwrap_or(false));
                                                                Effect::new(move |_| {
                                                                    let c = checked.get();
                                                                    let cur = current_val_b().as_bool().unwrap_or(false);
                                                                    if c != cur {
                                                                        on_change(Value::Bool(c));
                                                                    }
                                                                });
                                                                view! {
                                                                    <div class="md:col-span-2">
                                                                        <label class="flex items-center gap-2 cursor-pointer">
                                                                            <input type="checkbox" class="toggle"
                                                                                prop:checked=move || checked.get()
                                                                                on:change=move |ev: ev::Event| {
                                                                                    checked.set(event_target_checked(&ev));
                                                                                }
                                                                            />
                                                                            <span>{label}</span>
                                                                        </label>
                                                                        {if !description.is_empty() {
                                                                            Some(view! { <p class="text-sm opacity-70 mt-1">{description.clone()}</p> })
                                                                        } else { None }}
                                                                    </div>
                                                                }.into_any()
                                                            }
                                                            "integer" | "number" => {
                                                                let on_change = on_change.clone();
                                                                let input_val = RwSignal::new(match current_val() {
                                                                    Value::Number(n) => n.to_string(),
                                                                    _ => String::new(),
                                                                });
                                                                Effect::new(move |_| {
                                                                    let text = input_val.get();
                                                                    if let Ok(n) = text.parse::<f64>() {
                                                                        on_change(serde_json::json!(n));
                                                                    }
                                                                });
                                                                view! {
                                                                    <div>
                                                                        <label class="label"><span class="label-text font-bold">{label}</span></label>
                                                                        <input class="input input-bordered w-full"
                                                                            placeholder=placeholder
                                                                            prop:value=move || input_val.get()
                                                                            on:input=move |ev: ev::Event| input_val.set(event_target_value(&ev))
                                                                        />
                                                                        {if !description.is_empty() {
                                                                            Some(view! { <p class="text-sm opacity-70 mt-1">{description.clone()}</p> })
                                                                        } else { None }}
                                                                    </div>
                                                                }.into_any()
                                                            }
                                                            _ => {
                                                                let on_change = on_change.clone();
                                                                let is_key = field_name.contains("key") || field_name.contains("secret");
                                                                let input_val = RwSignal::new(
                                                                    current_val().as_str().unwrap_or("").to_string()
                                                                );
                                                                Effect::new(move |_| {
                                                                    let text = input_val.get();
                                                                    on_change(Value::String(text));
                                                                });
                                                                view! {
                                                                    <div>
                                                                        <label class="label"><span class="label-text font-bold">{label}</span></label>
                                                                        <input class="input input-bordered w-full"
                                                                            r#type={if is_key { "password" } else { "text" }}
                                                                            placeholder=placeholder
                                                                            prop:value=move || input_val.get()
                                                                            on:input=move |ev: ev::Event| input_val.set(event_target_value(&ev))
                                                                        />
                                                                        {if !description.is_empty() {
                                                                            Some(view! { <p class="text-sm opacity-70 mt-1">{description.clone()}</p> })
                                                                        } else { None }}
                                                                    </div>
                                                                }.into_any()
                                                            }
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }).collect_view()}

                                <div class="flex items-center gap-2 mt-4">
                                    <button class="btn btn-primary"
                                        on:click=move |_| {
                                            let data = form_data.get_untracked();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                match api::put::<Value>("/config", &data).await {
                                                    Ok(_) => save_msg.set(Some("Configuration saved".into())),
                                                    Err(e) => save_msg.set(Some(format!("Error: {e}"))),
                                                }
                                            });
                                        }
                                    >"Save"</button>
                                    {move || save_msg.get().map(|msg| {
                                        if msg.starts_with("Error") {
                                            view! {
                                                <div role="alert" class="alert alert-error">
                                                    <span>{msg}</span>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <span class="badge badge-success">{msg}</span> }.into_any()
                                        }
                                    })}
                                </div>
                            </div>
                        }.into_any()
                    }
                }}
            </Suspense>
        </div>
    }
}
