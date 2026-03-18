use std::collections::HashMap;

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use thaw::{ConfigProvider, Theme};

use crate::layout::AppLayout;
use crate::pages;

fn brand_colors() -> HashMap<i32, &'static str> {
    HashMap::from([
        (10, "#030108"), (20, "#150C27"), (30, "#231543"),
        (40, "#2F1B5E"), (50, "#3B227B"), (60, "#6B4EAB"),
        (70, "#7E62BB"), (80, "#9177CB"), (90, "#A48DDA"),
        (100, "#B7A3E8"), (110, "#C9B9F0"), (120, "#DACFF6"),
        (130, "#E8E1FA"), (140, "#F1EDFC"), (150, "#F8F6FE"),
        (160, "#FCFBFF"),
    ])
}

fn load_theme() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("agentcorp-theme").ok().flatten())
        .map(|v| v == "dark")
        .unwrap_or(true) // default to dark
}

#[component]
pub fn App() -> impl IntoView {
    let is_dark = RwSignal::new(load_theme());
    let colors = brand_colors();
    let theme_signal = RwSignal::new(if is_dark.get_untracked() {
        Theme::custom_dark(&colors)
    } else {
        Theme::custom_light(&colors)
    });

    let colors2 = brand_colors();
    Effect::new(move |_| {
        let t = if is_dark.get() {
            Theme::custom_dark(&colors2)
        } else {
            Theme::custom_light(&colors2)
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
