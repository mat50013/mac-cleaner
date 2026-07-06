//! Administrator elevation and Full Disk Access checks.

use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

/// System Settings deep link to the Full Disk Access pane.
pub const FDA_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

#[derive(Debug, Clone, Copy)]
pub struct PrivilegeInfo {
    pub is_root: bool,
    /// True when system and other-user paths may be unavailable.
    pub limited: bool,
    pub full_disk_access: bool,
}

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Login user that launched the process (set by `sudo` as `SUDO_USER`).
pub fn invoking_user() -> Option<String> {
    std::env::var("SUDO_USER")
        .ok()
        .filter(|u| is_valid_login_user(u))
}

fn is_valid_login_user(name: &str) -> bool {
    !name.is_empty() && name != "root"
}

/// Re-exec through `sudo` when elevation is enabled and the terminal can prompt.
pub fn maybe_elevate(auto_elevate: bool) -> PrivilegeInfo {
    let root = is_root();
    if root {
        return PrivilegeInfo {
            is_root: true,
            limited: false,
            full_disk_access: has_full_disk_access(),
        };
    }

    let can_prompt = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if auto_elevate && can_prompt && which("sudo") {
        if let Ok(exe) = std::env::current_exe() {
            let args: Vec<String> = std::env::args().skip(1).collect();
            eprintln!("mac-cleaner: requesting administrator privileges via sudo...");
            let status = Command::new("sudo").arg("--").arg(exe).args(&args).status();
            match status {
                Ok(s) => std::process::exit(s.code().unwrap_or(0)),
                Err(e) => {
                    eprintln!("mac-cleaner: sudo failed ({e}); continuing in limited mode.");
                }
            }
        }
    }

    PrivilegeInfo {
        is_root: false,
        limited: true,
        full_disk_access: has_full_disk_access(),
    }
}

/// Probe access to common TCC-protected locations.
pub fn has_full_disk_access() -> bool {
    let home = crate::fs_util::home_dir();
    let candidates = [
        home.join("Library/Application Support/com.apple.TCC/TCC.db"),
        home.join("Library/Mail"),
        home.join("Library/Safari"),
    ];
    for path in candidates {
        match probe_read(&path) {
            Some(true) => return true,
            Some(false) => return false,
            None => continue,
        }
    }
    true
}

/// `Some(true)` readable, `Some(false)` permission denied, `None` inconclusive.
fn probe_read(path: &Path) -> Option<bool> {
    if path.is_dir() {
        match std::fs::read_dir(path) {
            Ok(_) => Some(true),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Some(false),
            Err(_) => None,
        }
    } else if path.is_file() {
        match std::fs::File::open(path) {
            Ok(_) => Some(true),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Some(false),
            Err(_) => None,
        }
    } else {
        None
    }
}

/// Open the Full Disk Access pane in System Settings.
pub fn open_fda_settings() {
    let _ = Command::new("open").arg(FDA_SETTINGS_URL).status();
}

#[cfg(test)]
mod tests {
    use super::is_valid_login_user;

    #[test]
    fn invoking_user_rejects_root_and_empty() {
        assert!(!is_valid_login_user(""));
        assert!(!is_valid_login_user("root"));
        assert!(is_valid_login_user("matei"));
    }
}

/// Whether an executable is resolvable on `PATH` (via `command -v`).
fn which(bin: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
