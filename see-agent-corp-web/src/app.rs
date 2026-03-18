use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::layout::Layout;
use crate::pages;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <Layout>
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
            </Layout>
        </Router>
    }
}
