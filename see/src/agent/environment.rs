/// Collect desktop environment info (macOS).
///
/// Returns an `<ENVIRONMENT>` block string for injection into the user task.
/// On non-macOS or any error, returns an empty string.
#[cfg(target_os = "macos")]
pub async fn collect_environment(screen_width: u32, screen_height: u32) -> String {
    use tokio::process::Command;
    use tokio::time::{timeout, Duration};

    let timeout_dur = Duration::from_secs(crate::consts::ENVIRONMENT_TIMEOUT_SECS);

    let running_apps = timeout(timeout_dur, async {
        Command::new("osascript")
            .args([
                "-e",
                "tell app \"System Events\" to get name of every process whose background only is false",
            ])
            .output()
            .await
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_owned())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    let frontmost_app = timeout(timeout_dur, async {
        Command::new("osascript")
            .args([
                "-e",
                "tell app \"System Events\" to get name of first process whose frontmost is true",
            ])
            .output()
            .await
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_owned())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    let installed_apps = timeout(timeout_dur, async {
        Command::new("ls")
            .arg("/Applications/")
            .output()
            .await
            .ok()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .take(crate::consts::MAX_INSTALLED_APPS_LIST)
                    .map(|l| l.trim_end_matches(".app"))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    format!(
        "<ENVIRONMENT>\n\
         当前运行的应用: {running_apps}\n\
         最前面的应用: {frontmost_app}\n\
         已安装的应用: {installed_apps}\n\
         屏幕分辨率: {screen_width}x{screen_height} (逻辑像素)\n\
         </ENVIRONMENT>"
    )
}

/// Linux stub — returns empty string until implemented.
#[cfg(target_os = "linux")]
pub async fn collect_environment(_screen_width: u32, _screen_height: u32) -> String {
    String::new()
}

/// Fallback for other platforms.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub async fn collect_environment(_screen_width: u32, _screen_height: u32) -> String {
    String::new()
}
