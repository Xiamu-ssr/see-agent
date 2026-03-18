use leptos::prelude::*;
use serde_json::Value;

use crate::api;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_ref<'a>(schema: &'a Value, definitions: &'a Value) -> &'a Value {
    // Direct $ref
    if let Some(ref_path) = schema.get("$ref").and_then(|r| r.as_str()) {
        let name = ref_path.strip_prefix("#/definitions/").unwrap_or(ref_path);
        return definitions.get(name).unwrap_or(schema);
    }
    // allOf: [{$ref: ...}] wrapper (schemars default format)
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
    let (save_msg, set_save_msg) = signal::<Option<String>>(None);
    let (initialized, set_initialized) = signal(false);

    view! {
        <div class="page">
            <div class="page-header">
                <h2>"Configuration"</h2>
            </div>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || {
                    let schema = schema_res.get();
                    let config = config_res.get();
                    let defaults = defaults_res.get();

                    // Wait for all three
                    let (schema, config, defaults) = match (schema, config, defaults) {
                        (Some(s), Some(c), Some(d)) => {
                            match ((*s).clone(), (*c).clone(), (*d).clone()) {
                                (Some(s), Some(c), Some(d)) => (s, c, d),
                                _ => return view! { <p class="error">"Could not load configuration"</p> }.into_any(),
                            }
                        }
                        _ => return view! { <p>"Loading configuration..."</p> }.into_any(),
                    };

                    {
                                    // Initialize form data once
                                    if !initialized.get_untracked() {
                                        form_data.set(config.clone());
                                        set_initialized.set(true);
                                    }

                                    let definitions = schema.get("definitions").cloned().unwrap_or(Value::Object(Default::default()));
                                    let properties = schema.get("properties").and_then(|p| p.as_object()).cloned().unwrap_or_default();

                                    let sections: Vec<_> = properties.into_iter().collect();

                                    view! {
                                        <div class="config-form">
                                            {sections.into_iter().map(|(section_name, section_schema)| {
                                                let resolved = resolve_ref(&section_schema, &definitions).clone();
                                                let section_props = resolved.get("properties").and_then(|p| p.as_object()).cloned().unwrap_or_default();
                                                let section_label = format_section_name(&section_name);
                                                let defs = definitions.clone();
                                                let section_defaults = defaults.get(&section_name).cloned().unwrap_or_default();

                                                view! {
                                                    <fieldset class="schema-section">
                                                        <legend>{section_label}</legend>
                                                        <div class="schema-fields">
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
                                                                        let current_val_display = current_val.clone();
                                                                        let current_val_change = current_val.clone();
                                                                        view! {
                                                                            <div class="form-group">
                                                                                <label class="form-label">
                                                                                    <input
                                                                                        type="checkbox"
                                                                                        prop:checked=move || current_val_display().as_bool().unwrap_or(false)
                                                                                        on:change={
                                                                                            let oc = on_change.clone();
                                                                                            move |_| {
                                                                                                let cur = current_val_change().as_bool().unwrap_or(false);
                                                                                                oc(Value::Bool(!cur));
                                                                                            }
                                                                                        }
                                                                                    />
                                                                                    {label}
                                                                                </label>
                                                                                {if !description.is_empty() {
                                                                                    Some(view! { <span class="form-hint">{description.clone()}</span> })
                                                                                } else { None }}
                                                                            </div>
                                                                        }.into_any()
                                                                    }
                                                                    "integer" | "number" => {
                                                                        let on_change = on_change.clone();
                                                                        view! {
                                                                            <div class="form-group">
                                                                                <label class="form-label">{label}</label>
                                                                                <input
                                                                                    type="number"
                                                                                    class="form-input"
                                                                                    placeholder=placeholder
                                                                                    prop:value=move || {
                                                                                        let v = current_val();
                                                                                        match v {
                                                                                            Value::Number(n) => n.to_string(),
                                                                                            _ => String::new(),
                                                                                        }
                                                                                    }
                                                                                    on:input={
                                                                                        let oc = on_change.clone();
                                                                                        move |ev| {
                                                                                            let text = event_target_value(&ev);
                                                                                            if let Ok(n) = text.parse::<f64>() {
                                                                                                oc(serde_json::json!(n));
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                />
                                                                                {if !description.is_empty() {
                                                                                    Some(view! { <span class="form-hint">{description.clone()}</span> })
                                                                                } else { None }}
                                                                            </div>
                                                                        }.into_any()
                                                                    }
                                                                    _ => {
                                                                        // String (default)
                                                                        let on_change = on_change.clone();
                                                                        let is_key = field_name.contains("key") || field_name.contains("secret");
                                                                        let input_type = if is_key { "password" } else { "text" };
                                                                        view! {
                                                                            <div class="form-group">
                                                                                <label class="form-label">{label}</label>
                                                                                <input
                                                                                    type=input_type
                                                                                    class="form-input"
                                                                                    placeholder=placeholder
                                                                                    prop:value=move || {
                                                                                        current_val().as_str().unwrap_or("").to_string()
                                                                                    }
                                                                                    on:input={
                                                                                        let oc = on_change.clone();
                                                                                        move |ev| {
                                                                                            let text = event_target_value(&ev);
                                                                                            oc(Value::String(text));
                                                                                        }
                                                                                    }
                                                                                />
                                                                                {if !description.is_empty() {
                                                                                    Some(view! { <span class="form-hint">{description.clone()}</span> })
                                                                                } else { None }}
                                                                            </div>
                                                                        }.into_any()
                                                                    }
                                                                }
                                                            }).collect_view()}
                                                        </div>
                                                    </fieldset>
                                                }
                                            }).collect_view()}

                                            <div class="config-actions">
                                                <button
                                                    class="btn btn-primary"
                                                    on:click=move |_| {
                                                        let data = form_data.get_untracked();
                                                        wasm_bindgen_futures::spawn_local(async move {
                                                            match api::put::<Value>("/config", &data).await {
                                                                Ok(_) => set_save_msg.set(Some("Configuration saved".into())),
                                                                Err(e) => set_save_msg.set(Some(format!("Error: {e}"))),
                                                            }
                                                        });
                                                    }
                                                >"Save"</button>
                                                {move || save_msg.get().map(|msg| {
                                                    let cls = if msg.starts_with("Error") { "save-error" } else { "save-success" };
                                                    view! { <span class=cls>{msg}</span> }
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
