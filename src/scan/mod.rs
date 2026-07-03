//! Scan orchestrator and per-category scanners.

pub mod caches;
pub mod duplicates;
pub mod icloud;
pub mod large;
pub mod logs;
pub mod trash_cat;

use crate::config::{Config, Matchers};
use crate::event::{DiskInfo, WorkerMsg, WorkerSender};
use crate::fs_util::{dir_real_size, home_dir};
use crate::model::{Category, ScanItem, SafetyTier};
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;

/// Shared context handed to every scanner.
pub struct ScanContext {
    pub config: Arc<Config>,
    pub matchers: Matchers,
    pub tx: WorkerSender,
    pub categories: Vec<Category>,
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
    for cat in cats {
        let cfg = Arc::clone(&ctx.config);
        let tx = ctx.tx.clone();
        thread::spawn(move || {
            let matchers = cfg.matchers().expect("matchers");
            let ctx = ScanContext {
                config: cfg,
                matchers,
                tx,
                categories: vec![cat],
            };
            scan_category(cat, ctx);
        });
    }
}

fn scan_category(cat: Category, ctx: ScanContext) {
    ctx.tx.send(WorkerMsg::ScanStarted(cat));
    let result = match cat {
        Category::Caches => caches::scan(&ctx),
        Category::Logs => logs::scan(&ctx),
        Category::Duplicates => duplicates::scan(&ctx),
        Category::ICloud => icloud::scan(&ctx),
        Category::LargeFiles => large::scan(&ctx),
        Category::Trash => trash_cat::scan(&ctx),
    };
    match result {
        Ok(items) => {
            let bytes: u64 = items.iter().map(|i| i.real_bytes).sum();
            ctx.send_progress(cat, items.len(), bytes);
            ctx.tx.send(WorkerMsg::ScanDone { category: cat, items });
        }
        Err(e) => {
            ctx.tx
                .send(WorkerMsg::ScanSkipped {
                    category: cat,
                    reason: e.to_string(),
                });
        }
    }
}

pub fn read_disk_info() -> DiskInfo {
    let output = Command::new("df")
        .args(["-k", "/"])
        .output()
        .ok();
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

/// Build a human label from the path by walking up to find a meaningful parent.
pub fn label_for(path: &Path, suffix: &str) -> String {
    let home = home_dir();
    let display = path.strip_prefix(&home).unwrap_or(path);
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());
    if suffix.is_empty() {
        format!("{parent} — {}", display.display())
    } else {
        format!("{parent} — {suffix}")
    }
}

/// Parallel walk of `root`, calling `on_dir` for every directory and `on_file` for files.
/// Returns early from a branch when `on_dir` returns true (prune subtree).
pub fn walk_parallel(
    root: &Path,
    matchers: &Matchers,
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
        .ignore(false);
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
                if matchers.is_excluded_dir(&name) {
                    return ignore::WalkState::Skip;
                }
                if on_dir(path, &name) {
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
