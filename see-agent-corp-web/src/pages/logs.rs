use leptos::prelude::*;
use serde::Deserialize;
use thaw::*;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct LogEntry {
    time: String,
    level: String,
    message: String,
}

#[component]
pub fn Logs() -> impl IntoView {
    let logs = LocalResource::new(|| async {
        api::get::<Vec<LogEntry>>("/logs").await.unwrap_or_default()
    });

    view! {
        <div>
            <Body1><b>"Logs"</b></Body1>
            <Divider />
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || logs.get().map(|list| {
                    if list.is_empty() {
                        view! { <MessageBar><MessageBarBody>"No log entries"</MessageBarBody></MessageBar> }.into_any()
                    } else {
                        let items: Vec<_> = list.iter().cloned().collect();
                        view! {
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHeaderCell>"Time"</TableHeaderCell>
                                        <TableHeaderCell>"Level"</TableHeaderCell>
                                        <TableHeaderCell>"Message"</TableHeaderCell>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {items.into_iter().map(|e| {
                                        let time = e.time;
                                        let level_text = e.level.clone();
                                        let badge_color = match e.level.to_lowercase().as_str() {
                                            "error" => BadgeColor::Danger,
                                            "warn" => BadgeColor::Warning,
                                            "info" => BadgeColor::Brand,
                                            _ => BadgeColor::Informative,
                                        };
                                        let msg = e.message;
                                        view! {
                                            <TableRow>
                                                <TableCell><TableCellLayout><Caption1>{time}</Caption1></TableCellLayout></TableCell>
                                                <TableCell><TableCellLayout><Badge color=badge_color>{level_text}</Badge></TableCellLayout></TableCell>
                                                <TableCell><TableCellLayout><code>{msg}</code></TableCellLayout></TableCell>
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
