use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="text-center pt-16">
            <p class="text-4xl font-bold">"404"</p>
            <div class="divider"></div>
            <p class="text-sm opacity-70">"Page not found"</p>
            <br />
            <A href="/">
                <button class="btn btn-primary">"Back to Dashboard"</button>
            </A>
        </div>
    }
}
