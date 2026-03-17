use leptos::prelude::*;
use serde::Deserialize;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct ToolInfo {
    name: String,
    description: Option<String>,
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
                                            <td>{t.name}</td>
                                            <td>{t.description.unwrap_or_default()}</td>
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
