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
        <div class="h-full overflow-y-auto">
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
                                                            "object" => {
                                                                let sub_props = resolved_field.get("properties").and_then(|p| p.as_object()).cloned().unwrap_or_default();
                                                                let sub_defs = defs.clone();
                                                                let sub_defaults = default_val.clone();
                                                                let sn_obj = section_name.clone();
                                                                let fn_obj = field_name.clone();
                                                                view! {
                                                                    <div class="md:col-span-2">
                                                                        <label class="label"><span class="label-text font-bold">{label}</span></label>
                                                                        {if !description.is_empty() {
                                                                            Some(view! { <p class="text-sm opacity-70 mb-1">{description.clone()}</p> })
                                                                        } else { None }}
                                                                        <div class="pl-4 border-l-2 border-base-300 grid grid-cols-1 md:grid-cols-2 gap-3">
                                                                            {sub_props.into_iter().map(move |(sub_name, sub_schema)| {
                                                                                let resolved_sub = resolve_ref(&sub_schema, &sub_defs).clone();
                                                                                let sub_type = schema_type(&resolved_sub).to_string();
                                                                                let sub_label = format_section_name(&sub_name);
                                                                                let sub_desc = resolved_sub.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                                                                                let sub_default = sub_defaults.get(&sub_name).cloned().unwrap_or_default();
                                                                                let sn_sub = sn_obj.clone();
                                                                                let fn_sub = fn_obj.clone();
                                                                                let sn_sub2 = sn_obj.clone();
                                                                                let fn_sub2 = fn_obj.clone();
                                                                                let sub_name2 = sub_name.clone();

                                                                                let sub_current = {
                                                                                    let sn = sn_sub.clone();
                                                                                    let fn_ = fn_sub.clone();
                                                                                    let sn2 = sub_name.clone();
                                                                                    move || {
                                                                                        let data = form_data.get();
                                                                                        data.get(&sn).and_then(|s| s.get(&fn_)).and_then(|o| o.get(&sn2)).cloned().unwrap_or_default()
                                                                                    }
                                                                                };

                                                                                let sub_on_change = move |new_val: Value| {
                                                                                    let sn = sn_sub2.clone();
                                                                                    let fn_ = fn_sub2.clone();
                                                                                    let sfn = sub_name2.clone();
                                                                                    form_data.update(|data| {
                                                                                        if let Some(obj) = data.as_object_mut() {
                                                                                            let section = obj.entry(sn).or_insert(Value::Object(Default::default()));
                                                                                            if let Some(section_map) = section.as_object_mut() {
                                                                                                let field = section_map.entry(fn_).or_insert(Value::Object(Default::default()));
                                                                                                if let Some(field_map) = field.as_object_mut() {
                                                                                                    field_map.insert(sfn, new_val);
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                };

                                                                                let sub_placeholder = match &sub_default {
                                                                                    Value::String(s) => s.clone(),
                                                                                    Value::Number(n) => n.to_string(),
                                                                                    Value::Bool(b) => b.to_string(),
                                                                                    _ => String::new(),
                                                                                };

                                                                                match sub_type.as_str() {
                                                                                    "boolean" => {
                                                                                        let sub_on_change = sub_on_change.clone();
                                                                                        let init = sub_current().as_bool().unwrap_or(false);
                                                                                        view! {
                                                                                            <div class="md:col-span-2">
                                                                                                <label class="flex items-center gap-2 cursor-pointer">
                                                                                                    <input type="checkbox" class="toggle"
                                                                                                        checked=init
                                                                                                        on:change={
                                                                                                            let on_change = sub_on_change.clone();
                                                                                                            move |ev: ev::Event| {
                                                                                                                on_change(Value::Bool(event_target_checked(&ev)));
                                                                                                            }
                                                                                                        }
                                                                                                    />
                                                                                                    <span>{sub_label}</span>
                                                                                                </label>
                                                                                                {if !sub_desc.is_empty() {
                                                                                                    Some(view! { <p class="text-sm opacity-70 mt-1">{sub_desc.clone()}</p> })
                                                                                                } else { None }}
                                                                                            </div>
                                                                                        }.into_any()
                                                                                    }
                                                                                    "integer" | "number" => {
                                                                                        let sub_on_change = sub_on_change.clone();
                                                                                        let input_val = RwSignal::new(match sub_current() {
                                                                                            Value::Number(n) => n.to_string(),
                                                                                            _ => String::new(),
                                                                                        });
                                                                                        view! {
                                                                                            <div>
                                                                                                <label class="label"><span class="label-text font-bold">{sub_label}</span></label>
                                                                                                <input class="input input-bordered w-full"
                                                                                                    placeholder=sub_placeholder
                                                                                                    prop:value=move || input_val.get()
                                                                                                    on:input={
                                                                                                        let on_change = sub_on_change.clone();
                                                                                                        move |ev: ev::Event| {
                                                                                                            let text = event_target_value(&ev);
                                                                                                            input_val.set(text.clone());
                                                                                                            if let Ok(n) = text.parse::<f64>() {
                                                                                                                on_change(serde_json::json!(n));
                                                                                                            }
                                                                                                        }
                                                                                                    }
                                                                                                />
                                                                                                {if !sub_desc.is_empty() {
                                                                                                    Some(view! { <p class="text-sm opacity-70 mt-1">{sub_desc.clone()}</p> })
                                                                                                } else { None }}
                                                                                            </div>
                                                                                        }.into_any()
                                                                                    }
                                                                                    _ => {
                                                                                        let sub_on_change = sub_on_change.clone();
                                                                                        let input_val = RwSignal::new(
                                                                                            sub_current().as_str().unwrap_or("").to_string()
                                                                                        );
                                                                                        view! {
                                                                                            <div>
                                                                                                <label class="label"><span class="label-text font-bold">{sub_label}</span></label>
                                                                                                <input class="input input-bordered w-full"
                                                                                                    placeholder=sub_placeholder
                                                                                                    prop:value=move || input_val.get()
                                                                                                    on:input={
                                                                                                        let on_change = sub_on_change.clone();
                                                                                                        move |ev: ev::Event| {
                                                                                                            let text = event_target_value(&ev);
                                                                                                            input_val.set(text.clone());
                                                                                                            on_change(Value::String(text));
                                                                                                        }
                                                                                                    }
                                                                                                />
                                                                                                {if !sub_desc.is_empty() {
                                                                                                    Some(view! { <p class="text-sm opacity-70 mt-1">{sub_desc.clone()}</p> })
                                                                                                } else { None }}
                                                                                            </div>
                                                                                        }.into_any()
                                                                                    }
                                                                                }
                                                                            }).collect_view()}
                                                                        </div>
                                                                    </div>
                                                                }.into_any()
                                                            }
                                                            "array" => {
                                                                let on_change = on_change.clone();
                                                                let items: Vec<String> = match current_val() {
                                                                    Value::Array(arr) => arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect(),
                                                                    _ => vec![],
                                                                };
                                                                let list = RwSignal::new(items);
                                                                let new_item = RwSignal::new(String::new());
                                                                view! {
                                                                    <div class="md:col-span-2">
                                                                        <label class="label"><span class="label-text font-bold">{label}</span></label>
                                                                        {if !description.is_empty() {
                                                                            Some(view! { <p class="text-sm opacity-70 mb-1">{description.clone()}</p> })
                                                                        } else { None }}
                                                                        <div class="flex flex-col gap-1">
                                                                            {
                                                                                let on_change_list = on_change.clone();
                                                                                move || list.get().into_iter().enumerate().map(|(i, item)| {
                                                                                let on_change = on_change_list.clone();
                                                                                view! {
                                                                                    <div class="flex items-center gap-1">
                                                                                        <code class="text-xs flex-1 bg-base-200 p-1 rounded">{item}</code>
                                                                                        <button class="btn btn-xs btn-ghost" on:click=move |_| {
                                                                                            list.update(|l| { l.remove(i); });
                                                                                            let vals: Vec<Value> = list.get_untracked().into_iter().map(Value::String).collect();
                                                                                            on_change(Value::Array(vals));
                                                                                        }>"x"</button>
                                                                                    </div>
                                                                                }
                                                                            }).collect_view()}
                                                                        </div>
                                                                        <div class="flex items-center gap-1 mt-1">
                                                                            <input class="input input-bordered input-sm flex-1"
                                                                                placeholder="Add item..."
                                                                                prop:value=move || new_item.get()
                                                                                on:input=move |ev: ev::Event| new_item.set(event_target_value(&ev))
                                                                                on:keydown={
                                                                                    let on_change = on_change.clone();
                                                                                    move |ev: web_sys::KeyboardEvent| {
                                                                                        if ev.key() == "Enter" {
                                                                                            ev.prevent_default();
                                                                                            let val = new_item.get_untracked();
                                                                                            if !val.trim().is_empty() {
                                                                                                list.update(|l| l.push(val));
                                                                                                new_item.set(String::new());
                                                                                                let vals: Vec<Value> = list.get_untracked().into_iter().map(Value::String).collect();
                                                                                                on_change(Value::Array(vals));
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                }
                                                                            />
                                                                            <button class="btn btn-sm btn-ghost" on:click={
                                                                                let on_change = on_change.clone();
                                                                                move |_| {
                                                                                    let val = new_item.get_untracked();
                                                                                    if !val.trim().is_empty() {
                                                                                        list.update(|l| l.push(val));
                                                                                        new_item.set(String::new());
                                                                                        let vals: Vec<Value> = list.get_untracked().into_iter().map(Value::String).collect();
                                                                                        on_change(Value::Array(vals));
                                                                                    }
                                                                                }
                                                                            }>"+"</button>
                                                                        </div>
                                                                    </div>
                                                                }.into_any()
                                                            }
                                                            "boolean" => {
                                                                let on_change = on_change.clone();
                                                                let init = current_val().as_bool().unwrap_or(false);
                                                                view! {
                                                                    <div class="md:col-span-2">
                                                                        <label class="flex items-center gap-2 cursor-pointer">
                                                                            <input type="checkbox" class="toggle"
                                                                                checked=init
                                                                                on:change={
                                                                                    let on_change = on_change.clone();
                                                                                    move |ev: ev::Event| {
                                                                                        on_change(Value::Bool(event_target_checked(&ev)));
                                                                                    }
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
                                                                view! {
                                                                    <div>
                                                                        <label class="label"><span class="label-text font-bold">{label}</span></label>
                                                                        <input class="input input-bordered w-full"
                                                                            placeholder=placeholder
                                                                            prop:value=move || input_val.get()
                                                                            on:input={
                                                                                let on_change = on_change.clone();
                                                                                move |ev: ev::Event| {
                                                                                    let text = event_target_value(&ev);
                                                                                    input_val.set(text.clone());
                                                                                    if let Ok(n) = text.parse::<f64>() {
                                                                                        on_change(serde_json::json!(n));
                                                                                    }
                                                                                }
                                                                            }
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
                                                                view! {
                                                                    <div>
                                                                        <label class="label"><span class="label-text font-bold">{label}</span></label>
                                                                        <input class="input input-bordered w-full"
                                                                            r#type={if is_key { "password" } else { "text" }}
                                                                            placeholder=placeholder
                                                                            prop:value=move || input_val.get()
                                                                            on:input={
                                                                                let on_change = on_change.clone();
                                                                                move |ev: ev::Event| {
                                                                                    let text = event_target_value(&ev);
                                                                                    input_val.set(text.clone());
                                                                                    on_change(Value::String(text));
                                                                                }
                                                                            }
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
