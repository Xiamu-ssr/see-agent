use leptos::prelude::*;
use leptos_router::components::A;
use thaw::*;

#[component]
pub fn AppLayout(is_dark: RwSignal<bool>, children: Children) -> impl IntoView {
    let selected_nav = RwSignal::new(String::from("dashboard"));
    let drawer_open = RwSignal::new(false);

    // Persist theme
    Effect::new(move |_| {
        let dark = is_dark.get();
        if let Some(storage) = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
        {
            let _ = storage.set_item("agentcorp-theme", if dark { "dark" } else { "light" });
        }
    });

    view! {
        <Layout has_sider=true>
            <LayoutSider
                content_style="width:220px;min-height:100vh;padding:0;background:var(--colorNeutralBackground2);border-right:1px solid var(--colorNeutralStroke1)"
                class="desktop-sider"
            >
                <div class="sider-brand">
                    <Flex vertical=false justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                        <Body1><b>"see-agent-corp"</b></Body1>
                        <Switch checked=is_dark />
                    </Flex>
                </div>
                <div style="padding:8px 12px">
                    <NavDrawer selected_value=selected_nav>
                        <NavItem icon=icondata_ai::AiDashboardOutlined value="dashboard"><A href="/">"Dashboard"</A></NavItem>
                        <NavItem icon=icondata_ai::AiRobotOutlined value="agents"><A href="/agents">"Agents"</A></NavItem>
                        <NavItem icon=icondata_ai::AiTeamOutlined value="teams"><A href="/teams">"Teams"</A></NavItem>
                        <NavItem icon=icondata_ai::AiSettingOutlined value="config"><A href="/config">"Config"</A></NavItem>
                        <NavItem icon=icondata_ai::AiThunderboltOutlined value="skills"><A href="/skills">"Skills"</A></NavItem>
                        <NavItem icon=icondata_ai::AiToolOutlined value="tools"><A href="/tools">"Tools"</A></NavItem>
                        <NavItem icon=icondata_ai::AiApiOutlined value="mcp"><A href="/mcp">"MCP"</A></NavItem>
                        <NavItem icon=icondata_ai::AiFileTextOutlined value="logs"><A href="/logs">"Logs"</A></NavItem>
                    </NavDrawer>
                </div>
            </LayoutSider>
            <Layout>
                <LayoutHeader class="mobile-header">
                    <Button
                        appearance=ButtonAppearance::Subtle
                        on_click=move |_| drawer_open.set(!drawer_open.get_untracked())
                    >
                        {move || if drawer_open.get() { "\u{2715}" } else { "\u{2630}" }}
                    </Button>
                    <Body1><b>"see-agent-corp"</b></Body1>
                </LayoutHeader>
                <OverlayDrawer open=drawer_open>
                    <DrawerHeader>
                        <Flex vertical=false justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                            <Body1><b>"see-agent-corp"</b></Body1>
                            <Switch checked=is_dark />
                        </Flex>
                    </DrawerHeader>
                    <DrawerBody>
                        <NavDrawer selected_value=selected_nav>
                            <NavItem icon=icondata_ai::AiDashboardOutlined value="dashboard"><A href="/" on:click=move |_| drawer_open.set(false)>"Dashboard"</A></NavItem>
                            <NavItem icon=icondata_ai::AiRobotOutlined value="agents"><A href="/agents" on:click=move |_| drawer_open.set(false)>"Agents"</A></NavItem>
                            <NavItem icon=icondata_ai::AiTeamOutlined value="teams"><A href="/teams" on:click=move |_| drawer_open.set(false)>"Teams"</A></NavItem>
                            <NavItem icon=icondata_ai::AiSettingOutlined value="config"><A href="/config" on:click=move |_| drawer_open.set(false)>"Config"</A></NavItem>
                            <NavItem icon=icondata_ai::AiThunderboltOutlined value="skills"><A href="/skills" on:click=move |_| drawer_open.set(false)>"Skills"</A></NavItem>
                            <NavItem icon=icondata_ai::AiToolOutlined value="tools"><A href="/tools" on:click=move |_| drawer_open.set(false)>"Tools"</A></NavItem>
                            <NavItem icon=icondata_ai::AiApiOutlined value="mcp"><A href="/mcp" on:click=move |_| drawer_open.set(false)>"MCP"</A></NavItem>
                            <NavItem icon=icondata_ai::AiFileTextOutlined value="logs"><A href="/logs" on:click=move |_| drawer_open.set(false)>"Logs"</A></NavItem>
                        </NavDrawer>
                    </DrawerBody>
                </OverlayDrawer>
                <div style="padding:24px;min-width:0;max-width:1200px">
                    {children()}
                </div>
            </Layout>
        </Layout>
    }
}
