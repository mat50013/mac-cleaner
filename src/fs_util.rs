//! Filesystem helpers: sparse-aware sizing, timestamps, and path expansion.
//!
//! Reclaimable space is measured from allocated blocks (`st_blocks`), not from
//! logical file length. This keeps sparse files, such as Docker VM disks, from
//! being reported as larger than their real on-disk footprint.

use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Real on-disk usage in bytes, derived from allocated 512-byte blocks.
pub fn real_size(md: &Metadata) -> u64 {
    md.blocks().saturating_mul(512)
}

/// A stable `(device, inode)` identity used to detect hard links so shared
/// inodes are only counted once.
pub fn inode_id(md: &Metadata) -> (u64, u64) {
    (md.dev(), md.ino())
}

/// Whole days since the file was last accessed. Falls back to `0` if the
/// platform cannot report an access time in the future/clock skew case.
pub fn atime_days(md: &Metadata) -> u32 {
    let accessed = md.accessed().ok();
    days_since(accessed)
}

/// Creation (birth) time, if available. On macOS `Metadata::created` maps to
/// `st_birthtime`, which is exactly what we want for "keep the oldest".
pub fn birthtime(md: &Metadata) -> Option<SystemTime> {
    md.created().ok()
}

fn days_since(t: Option<SystemTime>) -> u32 {
    let Some(t) = t else { return 0 };
    match SystemTime::now().duration_since(t) {
        Ok(d) => (d.as_secs() / 86_400) as u32,
        Err(_) => 0,
    }
}

/// Elapsed duration since `t`, saturating at zero for clock skew.
pub fn age(t: SystemTime) -> Duration {
    SystemTime::now()
        .duration_since(t)
        .unwrap_or(Duration::ZERO)
}

/// The current user's home directory.
pub fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// The macOS user Trash directory (`~/.Trash`).
pub fn user_trash_dir() -> PathBuf {
    home_dir().join(".Trash")
}

/// True when `path` is inside the user's Trash folder (already trashed).
pub fn is_in_user_trash(path: &Path) -> bool {
    let trash = user_trash_dir();
    path.starts_with(&trash) && path != trash
}

pub fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        return home_dir();
    }
    if let Some(rest) = input.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(input)
}

/// Format a byte count as a compact human string, e.g. `1.3 GB`.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Recursively sum the real size of a path without following symlinks.
/// Hard-linked files are counted once per inode.
pub fn dir_real_size(path: &Path, seen: &mut std::collections::HashSet<(u64, u64)>) -> u64 {
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if md.file_type().is_symlink() {
        return 0;
    }
    if md.is_file() {
        let id = inode_id(&md);
        if md.nlink() > 1 && !seen.insert(id) {
            return 0;
        }
        return real_size(&md);
    }
    if !md.is_dir() {
        return 0;
    }
    let mut total = real_size(&md);
    let Ok(entries) = std::fs::read_dir(path) else {
        return total;
    };
    for entry in entries.flatten() {
        total = total.saturating_add(dir_real_size(&entry.path(), seen));
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_size_formats() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(1024 * 1024), "1.0 MB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn expand_tilde_expands() {
        let home = home_dir();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/Library"), home.join("Library"));
        assert_eq!(expand_tilde("/tmp/x"), PathBuf::from("/tmp/x"));
    }

    #[test]
    fn is_in_user_trash_detects_children_only() {
        let trash = user_trash_dir();
        assert!(is_in_user_trash(&trash.join("some-file")));
        assert!(!is_in_user_trash(&trash));
        assert!(!is_in_user_trash(Path::new("/tmp/foo")));
    }
}
