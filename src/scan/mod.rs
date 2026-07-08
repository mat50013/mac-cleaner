//! Scan orchestrator and per-category scanners.

pub mod caches;
pub mod dev_artifacts;
pub mod duplicates;
pub mod icloud;
pub mod large;
pub mod logs;
pub mod trash_cat;

use crate::config::{Config, Matchers};
use crate::event::{DiskInfo, WorkerMsg, WorkerSender};
use crate::fs_util::{dir_real_size, home_dir};
use crate::model::{Category, SafetyTier, ScanItem};
use ignore::WalkBuilder;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;

/// Concurrency budget for a scan run.
#[derive(Debug, Clone)]
pub struct ScanLimits {
    pub category_workers: usize,
    pub walk_threads: usize,
    pub hash_threads: usize,
}

impl ScanLimits {
    pub fn auto(category_count: usize) -> Self {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self::for_cpus(category_count, cpus)
    }

    fn for_cpus(category_count: usize, cpus: usize) -> Self {
        let cpus = cpus.max(1);
        let category_workers = category_count.max(1).min((cpus / 4).clamp(2, 4));
        let walk_threads = (cpus / (category_workers * 2)).clamp(1, 2);
        let hash_threads = if cpus <= 2 {
            1
        } else {
            cpus.saturating_sub(category_workers)
                .saturating_sub(1)
                .max(2)
        };

        ScanLimits {
            category_workers,
            walk_threads,
            hash_threads,
        }
    }
}

/// Shared context handed to every scanner.
pub struct ScanContext {
    pub config: Arc<Config>,
    pub matchers: Matchers,
    pub tx: WorkerSender,
    pub categories: Vec<Category>,
    pub limits: Arc<ScanLimits>,
}

impl ScanContext {
    pub fn send_progress(&self, category: Category, found: usize, bytes: u64) {
        self.tx.send(WorkerMsg::ScanProgress {
            category,
            found,
            bytes,
        });
    }
}

/// Kick off all category scans on background threads.
pub fn run_all(ctx: ScanContext) {
    let tx_disk = ctx.tx.clone();
    thread::spawn(move || {
        tx_disk.send(WorkerMsg::Disk(read_disk_info()));
    });

    let cats = ctx.categories.clone();
    let cfg = Arc::clone(&ctx.config);
    let tx = ctx.tx.clone();
    let limits = Arc::clone(&ctx.limits);

    thread::spawn(move || {
        let pool = ThreadPoolBuilder::new()
            .num_threads(limits.category_workers)
            .thread_name(|i| format!("mac-cleaner-scan-{i}"))
            .build();

        let Ok(pool) = pool else {
            for cat in cats {
                tx.send(WorkerMsg::ScanSkipped {
                    category: cat,
                    reason: "failed to create scan worker pool".into(),
                });
            }
            return;
        };

        pool.install(|| {
            cats.into_par_iter().for_each(|cat| {
                let matchers = match cfg.matchers() {
                    Ok(matchers) => matchers,
                    Err(err) => {
                        tx.send(WorkerMsg::ScanSkipped {
                            category: cat,
                            reason: err.to_string(),
                        });
                        return;
                    }
                };
                let ctx = ScanContext {
                    config: Arc::clone(&cfg),
                    matchers,
                    tx: tx.clone(),
                    categories: vec![cat],
                    limits: Arc::clone(&limits),
                };
                scan_category(cat, ctx);
            });
        });
    });
}

fn scan_category(cat: Category, ctx: ScanContext) {
    ctx.tx.send(WorkerMsg::ScanStarted(cat));
    let result = match cat {
        Category::Caches => caches::scan(&ctx),
        Category::Logs => logs::scan(&ctx),
        Category::DevArtifacts => dev_artifacts::scan(&ctx),
        Category::Duplicates => duplicates::scan(&ctx),
        Category::ICloud => icloud::scan(&ctx),
        Category::LargeFiles => large::scan(&ctx),
        Category::Trash => trash_cat::scan(&ctx),
    };
    match result {
        Ok(items) => {
            let bytes: u64 = items.iter().map(|i| i.real_bytes).sum();
            ctx.send_progress(cat, items.len(), bytes);
            ctx.tx.send(WorkerMsg::ScanDone {
                category: cat,
                items,
            });
        }
        Err(e) => {
            ctx.tx.send(WorkerMsg::ScanSkipped {
                category: cat,
                reason: e.to_string(),
            });
        }
    }
}

pub fn read_disk_info() -> DiskInfo {
    let output = Command::new("df").args(["-k", "/"]).output().ok();
    if let Some(out) = output {
        if let Ok(text) = String::from_utf8(out.stdout) {
            for line in text.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let total = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                    let free = parts[3].parse::<u64>().unwrap_or(0) * 1024;
                    return DiskInfo { total, free };
                }
            }
        }
    }
    DiskInfo::default()
}

/// Sum real bytes of a path (file or directory tree), deduplicating hard links.
pub fn path_bytes(path: &Path) -> u64 {
    let mut seen = HashSet::new();
    dir_real_size(path, &mut seen)
}

/// Build a display label from a path.
pub fn label_for(path: &Path, suffix: &str) -> String {
    let home = home_dir();
    let rel = path
        .strip_prefix(&home)
        .unwrap_or(path)
        .display()
        .to_string();
    let fname = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let is_file = std::fs::symlink_metadata(path)
        .map(|m| m.is_file())
        .unwrap_or(false);

    if suffix.is_empty() {
        let parent = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        format!("{parent} — {rel}")
    } else if suffix == fname || is_file || is_generic_category_suffix(suffix) {
        rel
    } else {
        format!("{rel} — {suffix}")
    }
}

fn is_generic_category_suffix(suffix: &str) -> bool {
    matches!(suffix, "large file" | "cache" | "logs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_for_file_uses_relative_path() {
        let home = home_dir();
        let path = home.join("Downloads/big-movie.mp4");
        assert_eq!(label_for(&path, "large file"), "Downloads/big-movie.mp4");
    }

    #[test]
    fn label_for_dir_uses_relative_path_for_cache() {
        let home = home_dir();
        let path = home.join("Library/Caches/com.example.app");
        assert_eq!(label_for(&path, "cache"), "Library/Caches/com.example.app");
    }

    #[test]
    fn label_for_trash_item_uses_relative_path() {
        let home = home_dir();
        let path = home.join(".Trash/old.dmg");
        assert_eq!(label_for(&path, "old.dmg"), ".Trash/old.dmg");
    }

    #[test]
    fn scan_limits_scale_for_fourteen_cores() {
        let limits = ScanLimits::for_cpus(7, 14);
        assert_eq!(limits.category_workers, 3);
        assert_eq!(limits.walk_threads, 2);
        assert_eq!(limits.hash_threads, 10);
    }
}

/// Parallel walk of `root`. Returning `true` from `on_dir` prunes that subtree.
pub fn walk_parallel(
    root: &Path,
    matchers: &Matchers,
    threads: usize,
    on_dir: impl Fn(&Path, &str) -> bool + Send + Sync,
    on_file: impl Fn(&Path, &str) + Send + Sync,
) {
    if !root.exists() {
        return;
    }
    let mut builder = WalkBuilder::new(root);
    builder
        .follow_links(false)
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .ignore(false)
        .threads(threads.max(1));
    builder.build_parallel().run(|| {
        Box::new(|entry| {
            let Ok(entry) = entry else {
                return ignore::WalkState::Continue;
            };
            let path = entry.path();
            let name = entry.file_name().to_string_lossy();
            if matchers.is_protected(path) {
                return ignore::WalkState::Skip;
            }
            let ft = entry.file_type();
            if ft.as_ref().is_some_and(|t| t.is_dir()) {
                if on_dir(path, &name) {
                    return ignore::WalkState::Skip;
                }
                if matchers.is_excluded_dir(&name) {
                    return ignore::WalkState::Skip;
                }
            } else if ft.as_ref().is_some_and(|t| t.is_file()) {
                on_file(path, &name);
            }
            ignore::WalkState::Continue
        })
    });
}

/// Create a ScanItem for a directory cache/log hit.
pub fn item_from_dir(
    path: PathBuf,
    label: String,
    bytes: u64,
    tier: SafetyTier,
    category: Category,
    note: &str,
) -> ScanItem {
    ScanItem::new(path, label, bytes, tier, category).with_note(note)
}

/// Check whether a CLI tool exists on PATH.
pub fn which(bin: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a subprocess and return stdout as a string.
pub fn run_cmd(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    if out.status.success() {
        String::from_utf8(out.stdout).ok()
    } else {
        None
    }
}
