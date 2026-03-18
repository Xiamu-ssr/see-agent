use leptos::prelude::*;
use leptos_router::components::A;
use thaw::*;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div style="text-align:center;padding-top:4rem">
            <Body1><b>"404"</b></Body1>
            <Divider />
            <Caption1>"Page not found"</Caption1>
            <br />
            <A href="/">
                <Button appearance=ButtonAppearance::Primary>"Back to Dashboard"</Button>
            </A>
        </div>
    }
}
