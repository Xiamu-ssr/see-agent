use see_agent_corp::supervisor::send_to_inbox_with_id;
use see_agent_corp::types::{Message, MessagePriority, WorkspaceDir};

pub fn run(workspace: &WorkspaceDir, agent_id: &str, content: &str, steer: bool) {
    let agent_dir = workspace.agent(agent_id);
    if !agent_dir.path().exists() {
        eprintln!("Agent '{agent_id}' not found");
        std::process::exit(1);
    }

    let priority = if steer {
        MessagePriority::Steer
    } else {
        MessagePriority::Collect
    };

    let msg = Message {
        msg_id: None,
        sender: "cli".into(),
        content: content.to_owned(),
        priority,
        metadata: Default::default(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    match send_to_inbox_with_id(&agent_dir.inbox(), msg) {
        Ok(()) => println!("Message sent to {agent_id}"),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
