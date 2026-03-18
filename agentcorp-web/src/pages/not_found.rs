use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="page not-found">
            <h2>"404"</h2>
            <p>"Page not found"</p>
            <A href="/">"Back to Dashboard"</A>
        </div>
    }
}
