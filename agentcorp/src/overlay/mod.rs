#[cfg(target_os = "macos")]
mod mac;
mod noop;
mod types;

#[cfg(target_os = "macos")]
pub use mac::MacOverlay;
pub use noop::NoopOverlay;
pub use types::DrawCommand;

/// Trait for rendering overlay indicators on screen.
pub trait Overlay: Send + Sync {
    /// Show a draw command on screen. The command auto-expires after its duration.
    fn show(&self, command: DrawCommand);

    /// Immediately dismiss all visible overlay content.
    fn dismiss(&self);
}

/// Create an overlay renderer appropriate for the current platform.
///
/// Returns a noop overlay if `enabled` is false or the platform is unsupported.
pub fn create_overlay(enabled: bool) -> Box<dyn Overlay> {
    if !enabled {
        return Box::new(NoopOverlay);
    }

    #[cfg(target_os = "macos")]
    {
        Box::new(MacOverlay::new())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Box::new(NoopOverlay)
    }
}
