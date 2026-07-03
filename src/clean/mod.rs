//! Deletion engine: trash, truncate, docker prune, iCloud evict, empty Trash.

use crate::config::DeleteMode;
use crate::event::{WorkerMsg, WorkerSender};
use crate::model::{ItemAction, ScanItem};
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
            // Synthetic/non-existent paths (e.g. the Docker prompt item) free
            // nothing; don't inflate the freed total.
            if !item.path.exists() {
                return Ok(0);
            }
            if opts.permanent || opts.mode == DeleteMode::Permanent {
                if item.path.is_dir() {
                    std::fs::remove_dir_all(&item.path).map_err(|e| e.to_string())?;
                } else {
                    std::fs::remove_file(&item.path).map_err(|e| e.to_string())?;
                }
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
            let status = Command::new("brctl")
                .args(["evict", &item.path.to_string_lossy()])
                .status()
                .map_err(|e| e.to_string())?;
            if status.success() {
                Ok(bytes)
            } else {
                Err("brctl evict failed".into())
            }
        }
        ItemAction::EmptyTrash => {
            empty_trash(&item.path)?;
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

fn empty_trash(trash_dir: &Path) -> Result<(), String> {
    if !trash_dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(trash_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
        } else {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
