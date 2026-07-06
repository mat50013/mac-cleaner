//! Cleaning engine.

use crate::config::DeleteMode;
use crate::event::{WorkerMsg, WorkerSender};
use crate::fs_util::{is_in_user_trash, user_trash_dir};
use crate::model::{Category, ItemAction, ScanItem};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::thread;

pub struct CleanOptions {
    pub permanent: bool,
    pub dry_run: bool,
    pub mode: DeleteMode,
}

pub fn run_clean(items: Vec<ScanItem>, opts: CleanOptions, tx: WorkerSender) {
    thread::spawn(move || {
        let total = items.len();
        let mut freed = 0u64;
        let mut failures = Vec::new();
        let mut done = 0usize;

        for item in &items {
            if opts.dry_run {
                freed = freed.saturating_add(item.real_bytes);
                done += 1;
                tx.send(WorkerMsg::CleanProgress { done, total, freed });
                continue;
            }
            match clean_one(item, &opts) {
                Ok(bytes) => freed = freed.saturating_add(bytes),
                Err(e) => failures.push(format!("{}: {e}", item.path.display())),
            }
            done += 1;
            tx.send(WorkerMsg::CleanProgress { done, total, freed });
        }

        tx.send(WorkerMsg::CleanDone { freed, failures });
    });
}

fn clean_one(item: &ScanItem, opts: &CleanOptions) -> Result<u64, String> {
    let bytes = item.real_bytes;
    match &item.action {
        ItemAction::Delete => {
            if !item.path.exists() {
                return Ok(0);
            }
            if should_delete_permanently(item, opts) {
                remove_path_permanently(&item.path)?;
            } else {
                trash::delete(&item.path).map_err(|e| e.to_string())?;
            }
            Ok(bytes)
        }
        ItemAction::Truncate => {
            OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&item.path)
                .map_err(|e| e.to_string())?
                .write_all(&[])
                .map_err(|e| e.to_string())?;
            Ok(bytes)
        }
        ItemAction::Evict => {
            evict_icloud(&item.path)?;
            Ok(bytes)
        }
        ItemAction::EmptyTrash => {
            empty_trash()?;
            Ok(bytes)
        }
        ItemAction::DockerPrune(kind) => {
            let args = kind.args();
            let status = Command::new("docker")
                .args(args)
                .status()
                .map_err(|e| e.to_string())?;
            if status.success() {
                Ok(bytes)
            } else {
                Err(format!("docker {} failed", args.join(" ")))
            }
        }
    }
}

/// `brctl evict` must run as the iCloud account owner, not root.
fn evict_icloud(path: &Path) -> Result<(), String> {
    let path_str = path.to_string_lossy();
    let output = if crate::privilege::is_root() {
        let user = crate::privilege::invoking_user().ok_or_else(|| {
            "iCloud evict cannot run as root (launch via sudo or use --no-elevate)".to_string()
        })?;
        Command::new("sudo")
            .args(["-u", &user, "brctl", "evict", &path_str])
            .output()
            .map_err(|e| e.to_string())?
    } else {
        Command::new("brctl")
            .args(["evict", &path_str])
            .output()
            .map_err(|e| e.to_string())?
    };
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err("brctl evict failed".into())
        } else {
            Err(stderr)
        }
    }
}

fn should_delete_permanently(item: &ScanItem, opts: &CleanOptions) -> bool {
    opts.permanent
        || opts.mode == DeleteMode::Permanent
        || item.category == Category::Trash
        || is_in_user_trash(&item.path)
}

fn remove_path_permanently(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        std::fs::remove_file(path).map_err(|e| e.to_string())
    }
}

/// Empty Trash through Finder, then fall back to removing `~/.Trash` contents.
fn empty_trash() -> Result<(), String> {
    if finder_empty_trash() {
        return Ok(());
    }
    empty_trash_dir(&user_trash_dir())
}

fn finder_empty_trash() -> bool {
    Command::new("osascript")
        .args([
            "-e",
            "try",
            "-e",
            "tell application \"Finder\" to empty trash",
            "-e",
            "end try",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn empty_trash_dir(trash_dir: &Path) -> Result<(), String> {
    if !trash_dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(trash_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        remove_path_permanently(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SafetyTier, ScanItem};
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn trash_category_always_permanent() {
        let item = ScanItem::new(
            std::path::PathBuf::from("/tmp/x"),
            "x",
            1,
            SafetyTier::Safe,
            Category::Trash,
        );
        let opts = CleanOptions {
            permanent: false,
            dry_run: false,
            mode: DeleteMode::Trash,
        };
        assert!(should_delete_permanently(&item, &opts));
    }

    #[test]
    fn remove_path_permanently_deletes_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("gone.txt");
        fs::File::create(&file).unwrap().write_all(b"x").unwrap();
        remove_path_permanently(&file).unwrap();
        assert!(!file.exists());
    }
}
