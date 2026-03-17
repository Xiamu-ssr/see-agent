mod capture;
mod scaling;

pub use capture::Eye;
pub use capture::Screenshot;
#[cfg(target_os = "macos")]
pub use capture::MacEye;
#[cfg(target_os = "linux")]
pub use capture::LinuxEye;
pub use scaling::{find_target_resolution, scale_coordinates, scale_screenshot, scale_tool_args};
