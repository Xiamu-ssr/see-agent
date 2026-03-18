use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct ToolInfo {
    name: String,
    description: String,
}

#[component]
pub fn Tools() -> impl IntoView {
    let tools = LocalResource::new(|| async {
        api::get::<Vec<ToolInfo>>("/tools").await.unwrap_or_default()
    });

    view! {
        <div>
            <h2 class="text-xl font-bold mb-4">"Tools"</h2>
            <Suspense fallback=|| view! { <span class="loading loading-spinner loading-lg"></span> }>
                {move || tools.get().map(|list| {
                    if list.is_empty() {
                        view! {
                            <div class="text-center py-12 opacity-60">
                                <p class="text-4xl mb-2">"🔧"</p>
                                <p>"No tools registered"</p>
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
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {items.into_iter().map(|t| {
                                            view! {
                                                <tr>
                                                    <td><code>{t.name}</code></td>
                                                    <td>{t.description}</td>
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
