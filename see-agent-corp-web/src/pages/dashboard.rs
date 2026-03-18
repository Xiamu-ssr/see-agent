use leptos::prelude::*;
use serde::Deserialize;
use thaw::*;

use crate::api;

#[derive(Debug, Clone, Deserialize)]
struct DashboardData {
    agents_count: usize,
    agents_running: usize,
    teams_count: usize,
    tools_count: usize,
    skills_count: usize,
    version: String,
}

#[component]
pub fn Dashboard() -> impl IntoView {
    let data = LocalResource::new(|| async {
        api::get::<DashboardData>("/dashboard").await.ok()
    });

    view! {
        <div>
            <Body1><b>"Dashboard"</b></Body1>
            <Divider />
            <Suspense fallback=|| view! { <Spinner /> }>
                {move || data.get().map(|d| {
                    match &*d {
                        Some(d) => {
                            let version = d.version.clone();
                            let agents_count = d.agents_count.to_string();
                            let agents_running = d.agents_running.to_string();
                            let teams_count = d.teams_count.to_string();
                            let tools_count = d.tools_count.to_string();
                            let skills_count = d.skills_count.to_string();
                            view! {
                                <Grid cols=3 x_gap=12 y_gap=12>
                                    <GridItem>
                                        <Card>
                                            <Caption1Strong>"Version"</Caption1Strong>
                                            <Body1>{version}</Body1>
                                        </Card>
                                    </GridItem>
                                    <GridItem>
                                        <Card>
                                            <Caption1Strong>"Total Agents"</Caption1Strong>
                                            <Body1>{agents_count}</Body1>
                                        </Card>
                                    </GridItem>
                                    <GridItem>
                                        <Card>
                                            <Caption1Strong>"Running"</Caption1Strong>
                                            <Badge color=BadgeColor::Success>{agents_running}</Badge>
                                        </Card>
                                    </GridItem>
                                    <GridItem>
                                        <Card>
                                            <Caption1Strong>"Teams"</Caption1Strong>
                                            <Body1>{teams_count}</Body1>
                                        </Card>
                                    </GridItem>
                                    <GridItem>
                                        <Card>
                                            <Caption1Strong>"Tools"</Caption1Strong>
                                            <Body1>{tools_count}</Body1>
                                        </Card>
                                    </GridItem>
                                    <GridItem>
                                        <Card>
                                            <Caption1Strong>"Skills"</Caption1Strong>
                                            <Body1>{skills_count}</Body1>
                                        </Card>
                                    </GridItem>
                                </Grid>
                            }.into_any()
                        }
                        None => view! {
                            <MessageBar>
                                <MessageBarBody>"Could not connect to server"</MessageBarBody>
                            </MessageBar>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_data_deserialize() {
        let json = r#"{
            "agents_count": 3,
            "agents_running": 1,
            "sleeping_agents": 2,
            "teams_count": 1,
            "tools_count": 10,
            "skills_count": 5,
            "version": "0.1.0"
        }"#;
        let d: DashboardData = serde_json::from_str(json).unwrap();
        assert_eq!(d.agents_count, 3);
        assert_eq!(d.agents_running, 1);
        assert_eq!(d.teams_count, 1);
        assert_eq!(d.tools_count, 10);
        assert_eq!(d.skills_count, 5);
        assert_eq!(d.version, "0.1.0");
    }

    #[test]
    fn dashboard_data_ignores_extra_fields() {
        let json = r#"{
            "agents_count": 1,
            "agents_running": 0,
            "teams_count": 0,
            "tools_count": 0,
            "skills_count": 0,
            "version": "0.1.0",
            "sleeping_agents": 5,
            "unknown_field": true
        }"#;
        let d: DashboardData = serde_json::from_str(json).unwrap();
        assert_eq!(d.agents_count, 1);
    }
}
