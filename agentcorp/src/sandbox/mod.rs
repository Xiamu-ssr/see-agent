mod collector;
mod generator;

pub use collector::{collect_violations, SandboxViolation};
pub use generator::generate_profile;
