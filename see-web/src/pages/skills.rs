use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct SkillInfo {
    name: String,
    description: String,
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
                    if list.is_empty() {
                        view! { <p class="empty">"No skills loaded"</p> }.into_any()
                    } else {
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
                                        let avail_class = if s.available { "avail-yes" } else { "avail-no" };
                                        view! {
                                            <tr>
                                                <td class="mono">{s.name}</td>
                                                <td>{s.description}</td>
                                                <td class=avail_class>{if s.available { "Yes" } else { "No" }}</td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        }.into_any()
                    }
                })}
            </Suspense>
        </div>
    }
}
