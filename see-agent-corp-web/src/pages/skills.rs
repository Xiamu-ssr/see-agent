use leptos::prelude::*;
use serde::Deserialize;
use thaw::*;

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
        <div class="page-content">
            <span class="page-header">"Skills"</span>
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || skills.get().map(|list| {
                    if list.is_empty() {
                        view! {
                            <div class="empty-state">
                                <div class="empty-state-icon">"⚡"</div>
                                <div class="empty-state-text">"No skills loaded"</div>
                            </div>
                        }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHeaderCell>"Name"</TableHeaderCell>
                                        <TableHeaderCell>"Description"</TableHeaderCell>
                                        <TableHeaderCell>"Available"</TableHeaderCell>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {items.into_iter().map(|s| {
                                        let name = s.name;
                                        let desc = s.description;
                                        let badge_color = if s.available { BadgeColor::Success } else { BadgeColor::Danger };
                                        let badge_text = if s.available { "Yes" } else { "No" };
                                        view! {
                                            <TableRow>
                                                <TableCell><TableCellLayout><code>{name}</code></TableCellLayout></TableCell>
                                                <TableCell><TableCellLayout>{desc}</TableCellLayout></TableCell>
                                                <TableCell><TableCellLayout><Badge color=badge_color>{badge_text}</Badge></TableCellLayout></TableCell>
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
