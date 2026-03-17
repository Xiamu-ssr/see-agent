use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct SkillInfo {
    name: String,
    description: Option<String>,
    available: bool,
}

#[component]
pub fn Skills() -> impl IntoView {
    let skills = LocalResource::new(|| async {
        api::get::<Vec<SkillInfo>>("/skills").await.unwrap_or_default()
    });

    view! {
        <div class="page">
            <h2>"Skills"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || skills.get().map(|list| {
                    let items: Vec<_> = list.iter().cloned().collect();
                    view! {
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th>"Name"</th>
                                    <th>"Description"</th>
                                    <th>"Available"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {items.into_iter().map(|s| {
                                    view! {
                                        <tr>
                                            <td>{s.name}</td>
                                            <td>{s.description.unwrap_or_default()}</td>
                                            <td>{if s.available { "Yes" } else { "No" }}</td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    }
                })}
            </Suspense>
        </div>
    }
}
