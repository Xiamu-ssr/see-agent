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
        <div class="page">
            <h2>"Tools"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || tools.get().map(|list| {
                    if list.is_empty() {
                        view! { <p class="empty">"No tools registered"</p> }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <table class="data-table">
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
                                                <td class="mono">{t.name}</td>
                                                <td>{t.description}</td>
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
