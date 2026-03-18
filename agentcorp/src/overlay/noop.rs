use super::types::DrawCommand;
use super::Overlay;

/// No-op overlay that does nothing. Used when overlay is disabled
/// or on unsupported platforms.
pub struct NoopOverlay;

impl Overlay for NoopOverlay {
    fn show(&self, _command: DrawCommand) {}
    fn dismiss(&self) {}
}
