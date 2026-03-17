/// Commands sent to the overlay renderer.
#[derive(Debug, Clone)]
pub enum DrawCommand {
    /// Show a click indicator at (x, y). If `double` is true, show double-click style.
    Click { x: f64, y: f64, double: bool },
    /// Show text being typed.
    Type { text: String },
    /// Show a drag arrow from (x1,y1) to (x2,y2).
    Drag {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    /// Show a scroll indicator at (x, y).
    Scroll {
        x: f64,
        y: f64,
        direction: String,
        amount: i32,
    },
    /// Show a hotkey combination.
    Hotkey { keys: Vec<String> },
    /// Show a shell command being executed.
    Shell { command: String },
    /// Show a wait/timer indicator.
    Wait { seconds: f64 },
    /// Flash the screen border to indicate screenshot.
    Screenshot,
    /// Show a question being asked to the user.
    CallUser { question: String },
    /// Show task completion banner.
    Finished { summary: String },
}

impl DrawCommand {
    /// Duration in seconds before this command auto-expires.
    pub fn duration(&self) -> f64 {
        match self {
            DrawCommand::Click { .. } => 1.0,
            DrawCommand::Type { .. } => 1.0,
            DrawCommand::Drag { .. } => 1.5,
            DrawCommand::Scroll { .. } => 1.0,
            DrawCommand::Hotkey { .. } => 1.0,
            DrawCommand::Shell { .. } => 1.5,
            DrawCommand::Wait { seconds, .. } => seconds + 0.5,
            DrawCommand::Screenshot => 0.3,
            DrawCommand::CallUser { .. } => 60.0,
            DrawCommand::Finished { .. } => 2.0,
        }
    }

    /// Short label for the command kind.
    pub fn kind(&self) -> &'static str {
        match self {
            DrawCommand::Click { .. } => "click",
            DrawCommand::Type { .. } => "type",
            DrawCommand::Drag { .. } => "drag",
            DrawCommand::Scroll { .. } => "scroll",
            DrawCommand::Hotkey { .. } => "hotkey",
            DrawCommand::Shell { .. } => "shell",
            DrawCommand::Wait { .. } => "wait",
            DrawCommand::Screenshot => "screenshot",
            DrawCommand::CallUser { .. } => "call_user",
            DrawCommand::Finished { .. } => "finished",
        }
    }
}
