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
        <div>
            <h2 class="text-xl font-bold mb-4">"Skills"</h2>
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || skills.get().map(|list| {
                    if list.is_empty() {
                        view! {
                            <div class="text-center py-12 opacity-60">
                                <p class="text-4xl mb-2">"⚡"</p>
                                <p>"No skills loaded"</p>
                            </div>
                        }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <div class="overflow-x-auto">
                                <table class="table">
                                    <thead>
                                        <tr>
                                            <th>"Name"</th>
                                            <th>"Description"</th>
                                            <th>"Available"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {items.into_iter().map(|s| {
                                            let badge_class = if s.available { "badge badge-success" } else { "badge badge-error" };
                                            let badge_text = if s.available { "Yes" } else { "No" };
                                            view! {
                                                <tr>
                                                    <td><code>{s.name}</code></td>
                                                    <td>{s.description}</td>
                                                    <td><span class=badge_class>{badge_text}</span></td>
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
