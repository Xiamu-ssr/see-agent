use leptos::prelude::*;
use serde::Deserialize;
use thaw::*;

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
            <Body1><b>"Tools"</b></Body1>
            <Divider />
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || tools.get().map(|list| {
                    if list.is_empty() {
                        view! { <MessageBar><MessageBarBody>"No tools registered"</MessageBarBody></MessageBar> }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHeaderCell>"Name"</TableHeaderCell>
                                        <TableHeaderCell>"Description"</TableHeaderCell>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {items.into_iter().map(|t| {
                                        let name = t.name;
                                        let desc = t.description;
                                        view! {
                                            <TableRow>
                                                <TableCell><TableCellLayout><code>{name}</code></TableCellLayout></TableCell>
                                                <TableCell><TableCellLayout>{desc}</TableCellLayout></TableCell>
                                            </TableRow>
                                        }
                                    }).collect_view()}
                                </TableBody>
                            </Table>
                        }.into_any()
                    }
                })}
            </Suspense>
        </div>
    }
}
