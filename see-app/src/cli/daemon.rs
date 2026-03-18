use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use see::consts::{
    DAEMON_RESTART_WAIT_MS, DAEMON_STARTUP_WAIT_MS, DAEMON_STOP_MAX_POLLS, DAEMON_STOP_POLL_MS,
    DEFAULT_SERVER_PORT,
};
use see::types::WorkspaceDir;

/// Write the current process ID to a PID file.
pub fn write_pid(path: &Path) {
    fs::write(path, std::process::id().to_string()).expect("failed to write PID file");
}

/// Read a PID from a PID file. Returns None if the file doesn't exist or is invalid.
pub fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// Remove the PID file.
pub fn remove_pid(path: &Path) {
    let _ = fs::remove_file(path);
}

/// Check if a process with the given PID is still alive.
#[cfg(unix)]
pub fn is_process_alive(pid: u32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    // Signal 0 doesn't send a signal but checks if the process exists
    kill(Pid::from_raw(pid as i32), None).is_ok()
}

#[cfg(not(unix))]
pub fn is_process_alive(_pid: u32) -> bool {
    false
}

/// Check for a stale PID file. Returns Some(pid) if process is genuinely running,
/// None if no server is running (removes stale PID file if found).
pub fn check_stale_pid(path: &Path) -> Option<u32> {
    let pid = read_pid(path)?;
    if is_process_alive(pid) {
        Some(pid)
    } else {
        remove_pid(path);
        None
    }
}

/// Start the server as a background daemon.
pub fn start(workspace: &WorkspaceDir, port: Option<u16>) {
    let pid_path = workspace.server_pid();
    if let Some(pid) = check_stale_pid(&pid_path) {
        eprintln!("Server already running (PID {pid})");
        std::process::exit(1);
    }

    let exe = std::env::current_exe().expect("cannot determine executable path");
    let log_file =
        fs::File::create(workspace.server_log()).expect("cannot create server.log");

    let port_val = port.unwrap_or(DEFAULT_SERVER_PORT);
    let port_str = port_val.to_string();
    let pid_str = pid_path.to_string_lossy().to_string();

    let mut cmd = Command::new(exe);
    cmd.args(["serve", "--port", &port_str, "--pid-file", &pid_str]);
    cmd.stdin(Stdio::null());
    cmd.stdout(log_file.try_clone().expect("failed to clone log file handle"));
    cmd.stderr(log_file);

    // On Unix, detach from controlling terminal
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    let mut child = cmd.spawn().expect("failed to start server");
    let child_pid = child.id();
    println!("Server starting (PID {child_pid})...");

    // Detach: we intentionally don't wait on the daemon child.
    // The child runs independently after setsid(). We just need to
    // reap it from our process table so it doesn't become a zombie
    // in the brief window before we exit.
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    // Brief wait then verify
    std::thread::sleep(std::time::Duration::from_millis(DAEMON_STARTUP_WAIT_MS));
    if let Some(pid) = check_stale_pid(&pid_path) {
        println!("Server running (PID {pid}), port {port_val}");
    } else {
        eprintln!("Warning: server may have failed to start. Check ~/.see-agent/server.log");
    }
}

/// Stop the running daemon.
pub fn stop(workspace: &WorkspaceDir) {
    let pid_path = workspace.server_pid();
    let Some(pid) = check_stale_pid(&pid_path) else {
        println!("No server running");
        return;
    };

    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        if let Err(e) = kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
            eprintln!("Failed to send SIGTERM: {e}");
            std::process::exit(1);
        }
    }

    // Wait for process to exit (up to 5 seconds)
    for _ in 0..DAEMON_STOP_MAX_POLLS {
        if !is_process_alive(pid) {
            remove_pid(&pid_path);
            println!("Server stopped (PID {pid})");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(DAEMON_STOP_POLL_MS));
    }

    // Force kill if still running
    eprintln!("Server did not stop gracefully, sending SIGKILL");
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
    }
    remove_pid(&pid_path);
    println!("Server killed (PID {pid})");
}

/// Restart the daemon (stop + start).
pub fn restart(workspace: &WorkspaceDir, port: Option<u16>) {
    let pid_path = workspace.server_pid();
    if check_stale_pid(&pid_path).is_some() {
        stop(workspace);
        // Brief wait for port to be released
        std::thread::sleep(std::time::Duration::from_millis(DAEMON_RESTART_WAIT_MS));
    }
    start(workspace, port);
}
