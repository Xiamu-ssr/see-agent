use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::layout::AppLayout;
use crate::pages;

fn load_theme() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("agentcorp-theme").ok().flatten())
        .map(|v| v == "dark")
        .unwrap_or(true) // default to dark
}

fn set_data_theme(dark: bool) {
    if let Some(doc) = web_sys::window().and_then(|w| w.document())
        && let Some(el) = doc.document_element()
    {
        let theme = if dark { "dim" } else { "winter" };
        let _ = el.set_attribute("data-theme", theme);
    }
}

#[component]
pub fn App() -> impl IntoView {
    let is_dark = RwSignal::new(load_theme());

    // Apply initial theme
    set_data_theme(is_dark.get_untracked());

    // React to theme changes
    Effect::new(move |_| {
        let dark = is_dark.get();
        set_data_theme(dark);
        // Persist
        if let Some(storage) = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
        {
            let _ = storage.set_item("agentcorp-theme", if dark { "dark" } else { "light" });
        }
    });

    view! {
        <Router>
            <AppLayout is_dark=is_dark>
                <Routes fallback=|| pages::NotFound>
                    <Route path=path!("/") view=pages::Dashboard />
                    <Route path=path!("/agents") view=pages::Agents />
                    <Route path=path!("/agents/:id") view=pages::AgentDetail />
                    <Route path=path!("/teams") view=pages::Teams />
                    <Route path=path!("/teams/:id") view=pages::TeamDetail />
                    <Route path=path!("/config") view=pages::Config />
                    <Route path=path!("/skills") view=pages::Skills />
                    <Route path=path!("/tools") view=pages::Tools />
                    <Route path=path!("/logs") view=pages::Logs />
                    <Route path=path!("/mcp") view=pages::Mcp />
                </Routes>
            </AppLayout>
        </Router>
    }
}
