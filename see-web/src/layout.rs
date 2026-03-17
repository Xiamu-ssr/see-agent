use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn Layout(children: Children) -> impl IntoView {
    let (dark, set_dark) = signal(load_theme());

    // Apply theme class to body
    Effect::new(move |_| {
        if let Some(doc) = document().document_element() {
            let _ = doc.set_attribute("data-theme", if dark.get() { "dark" } else { "light" });
        }
        save_theme(dark.get());
    });

    view! {
        <div class="app-layout">
            <nav class="sidebar">
                <div class="sidebar-header">
                    <h1 class="logo">"see-agent"</h1>
                    <button
                        class="theme-toggle"
                        on:click=move |_| set_dark.update(|d| *d = !*d)
                    >
                        {move || if dark.get() { "Light" } else { "Dark" }}
                    </button>
                </div>
                <ul class="nav-links">
                    <li><A href="/">"Dashboard"</A></li>
                    <li><A href="/agents">"Agents"</A></li>
                    <li><A href="/teams">"Teams"</A></li>
                    <li><A href="/config">"Config"</A></li>
                    <li><A href="/skills">"Skills"</A></li>
                    <li><A href="/tools">"Tools"</A></li>
                    <li><A href="/mcp">"MCP"</A></li>
                    <li><A href="/logs">"Logs"</A></li>
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
        .and_then(|s| s.get_item("see-theme").ok().flatten())
        .map(|v| v == "dark")
        .unwrap_or(true)
}

fn save_theme(dark: bool) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
    {
        let _ = storage.set_item("see-theme", if dark { "dark" } else { "light" });
    }
}
