use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn Layout(children: Children) -> impl IntoView {
    let (dark, set_dark) = signal(load_theme());
    let (sidebar_open, set_sidebar_open) = signal(false);

    // Apply theme class to body
    Effect::new(move |_| {
        if let Some(doc) = document().document_element() {
            let _ = doc.set_attribute("data-theme", if dark.get() { "dark" } else { "light" });
        }
        save_theme(dark.get());
    });

    let close_sidebar = move |_| set_sidebar_open.set(false);

    view! {
        <div class="app-layout">
            <button
                class="hamburger"
                on:click=move |_| set_sidebar_open.update(|v| *v = !*v)
            >
                {move || if sidebar_open.get() { "\u{2715}" } else { "\u{2630}" }}
            </button>

            <div
                class=move || if sidebar_open.get() { "sidebar-overlay visible" } else { "sidebar-overlay" }
                on:click=close_sidebar
            />

            <nav class=move || if sidebar_open.get() { "sidebar open" } else { "sidebar" }>
                <div class="sidebar-header">
                    <h1 class="logo">"see-agent-corp"</h1>
                    <button
                        class="theme-toggle"
                        on:click=move |_| set_dark.update(|d| *d = !*d)
                    >
                        {move || if dark.get() { "Light" } else { "Dark" }}
                    </button>
                </div>
                <ul class="nav-links">
                    <li><A href="/" on:click=close_sidebar>"Dashboard"</A></li>
                    <li><A href="/agents" on:click=close_sidebar>"Agents"</A></li>
                    <li><A href="/teams" on:click=close_sidebar>"Teams"</A></li>
                    <li><A href="/config" on:click=close_sidebar>"Config"</A></li>
                    <li><A href="/skills" on:click=close_sidebar>"Skills"</A></li>
                    <li><A href="/tools" on:click=close_sidebar>"Tools"</A></li>
                    <li><A href="/mcp" on:click=close_sidebar>"MCP"</A></li>
                    <li><A href="/logs" on:click=close_sidebar>"Logs"</A></li>
                </ul>
            </nav>
            <main class="content">
                {children()}
            </main>
        </div>
    }
}

fn load_theme() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("agentcorp-theme").ok().flatten())
        .map(|v| v == "dark")
        .unwrap_or(true)
}

fn save_theme(dark: bool) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
    {
        let _ = storage.set_item("agentcorp-theme", if dark { "dark" } else { "light" });
    }
}
