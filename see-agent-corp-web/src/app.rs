use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use thaw::{ConfigProvider, Theme};

use crate::layout::AppLayout;
use crate::pages;

fn load_theme() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("agentcorp-theme").ok().flatten())
        .map(|v| v == "dark")
        .is_some_and(|v| v)
}

#[component]
pub fn App() -> impl IntoView {
    let is_dark = RwSignal::new(load_theme());
    let theme_signal = RwSignal::new(if is_dark.get_untracked() {
        Theme::dark()
    } else {
        Theme::light()
    });

    Effect::new(move |_| {
        let t = if is_dark.get() {
            Theme::dark()
        } else {
            Theme::light()
        };
        theme_signal.set(t);
    });

    view! {
        <ConfigProvider theme=theme_signal>
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
        </ConfigProvider>
    }
}
