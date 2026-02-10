//! Signal handling for hsab shell
//!
//! Provides signal handling infrastructure for job control:
//! - SIGTSTP (Ctrl+Z): Suspend foreground job
//! - SIGCONT: Resume suspended job
//! - SIGCHLD: Reap child processes

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

#[cfg(unix)]
use nix::sys::signal::{kill, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

/// PID of the current foreground process (or -1 if none)
pub static FOREGROUND_PID: AtomicI32 = AtomicI32::new(-1);

/// Flag indicating SIGTSTP was received (set by signal handler)
pub static SIGTSTP_RECEIVED: AtomicBool = AtomicBool::new(false);

/// Set up signal handlers for the shell
#[cfg(unix)]
pub fn setup_signal_handlers() {
    use signal_hook::low_level;

    // Register SIGTSTP handler that sets the flag
    unsafe {
        let _ = low_level::register(signal_hook::consts::SIGTSTP, || {
            SIGTSTP_RECEIVED.store(true, Ordering::SeqCst);
        });
    }
}

/// Set up signal handlers (no-op on non-Unix)
#[cfg(not(unix))]
pub fn setup_signal_handlers() {}

/// Set the foreground process PID
pub fn set_foreground_pid(pid: i32) {
    FOREGROUND_PID.store(pid, Ordering::SeqCst);
}

/// Clear the foreground process PID
pub fn clear_foreground_pid() {
    FOREGROUND_PID.store(-1, Ordering::SeqCst);
}

/// Get the current foreground process PID (or None if no foreground job)
pub fn get_foreground_pid() -> Option<i32> {
    let pid = FOREGROUND_PID.load(Ordering::SeqCst);
    if pid > 0 {
        Some(pid)
    } else {
        None
    }
}

/// Check if SIGTSTP was received and clear the flag
pub fn check_sigtstp() -> bool {
    SIGTSTP_RECEIVED.swap(false, Ordering::SeqCst)
}

/// Send SIGSTOP to a process
#[cfg(unix)]
pub fn stop_process(pid: u32) -> Result<(), String> {
    let pid = Pid::from_raw(pid as i32);
    kill(pid, Signal::SIGSTOP)
        .map_err(|e| format!("Failed to stop process {}: {}", pid, e))
}

#[cfg(not(unix))]
pub fn stop_process(_pid: u32) -> Result<(), String> {
    Err("Signal handling not supported on this platform".into())
}

/// Send SIGCONT to a process
#[cfg(unix)]
pub fn continue_process(pid: u32) -> Result<(), String> {
    let pid = Pid::from_raw(pid as i32);
    kill(pid, Signal::SIGCONT)
        .map_err(|e| format!("Failed to continue process {}: {}", pid, e))
}

#[cfg(not(unix))]
pub fn continue_process(_pid: u32) -> Result<(), String> {
    Err("Signal handling not supported on this platform".into())
}

/// Send SIGTERM to a process
#[cfg(unix)]
pub fn terminate_process(pid: u32) -> Result<(), String> {
    let pid = Pid::from_raw(pid as i32);
    kill(pid, Signal::SIGTERM)
        .map_err(|e| format!("Failed to terminate process {}: {}", pid, e))
}

#[cfg(not(unix))]
pub fn terminate_process(_pid: u32) -> Result<(), String> {
    Err("Signal handling not supported on this platform".into())
}
