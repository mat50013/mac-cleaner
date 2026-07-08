//! Log scanner.

use crate::fs_util::atime_days;
use crate::model::{Category, ItemAction, SafetyTier, ScanItem};
use crate::scan::{ScanContext, item_from_dir, label_for, path_bytes, walk_parallel};
use anyhow::Result;
use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::Mutex;

pub fn scan(ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let mut items = Vec::new();
    let seen = Mutex::new(HashSet::<PathBuf>::new());
    let items_mtx = Mutex::new(Vec::<ScanItem>::new());
    let age_days = ctx.config.logs.age_days;

    for root in ctx.config.log_roots() {
        let matchers = &ctx.matchers;
        walk_parallel(
            &root,
            matchers,
            ctx.limits.walk_threads,
            |path, name| {
                if matchers.is_log_dir(name) {
                    {
                        let mut guard = seen.lock().unwrap();
                        if !guard.insert(path.to_path_buf()) {
                            return true;
                        }
                    }
                    let bytes = path_bytes(path);
                    if bytes > 0 {
                        let days = dir_max_atime_days(path);
                        let tier = if days >= age_days {
                            SafetyTier::Safe
                        } else {
                            SafetyTier::Moderate
                        };
                        items_mtx.lock().unwrap().push(
                            item_from_dir(
                                path.to_path_buf(),
                                label_for(path, "logs"),
                                bytes,
                                tier,
                                Category::Logs,
                                if days >= age_days {
                                    "old logs"
                                } else {
                                    "recent logs — review before deleting"
                                },
                            )
                            .with_age(days),
                        );
                    }
                    return true;
                }
                false
            },
            |path, name| {
                if !matchers.is_log_file(name) {
                    return;
                }
                let mut guard = seen.lock().unwrap();
                if !guard.insert(path.to_path_buf()) {
                    return;
                }
                let Ok(md) = std::fs::symlink_metadata(path) else {
                    return;
                };
                let bytes = md.blocks() * 512;
                if bytes == 0 {
                    return;
                }
                let days = atime_days(&md);
                let tier = if days >= age_days {
                    SafetyTier::Safe
                } else {
                    SafetyTier::Moderate
                };
                let mut item = ScanItem::new(
                    path.to_path_buf(),
                    label_for(path, &name),
                    bytes,
                    tier,
                    Category::Logs,
                )
                .with_age(days)
                .with_note("log file");
                if ctx.config.logs.truncate_active && days < age_days {
                    item.action = ItemAction::Truncate;
                }
                items_mtx.lock().unwrap().push(item);
            },
        );
    }

    items.append(&mut items_mtx.into_inner().unwrap());

    let sys_log = PathBuf::from("/private/var/log");
    if sys_log.is_dir() && crate::privilege::is_root() {
        let bytes = path_bytes(&sys_log);
        if bytes > 0 {
            items.push(
                ScanItem::new(
                    sys_log,
                    "System logs (/private/var/log)",
                    bytes,
                    SafetyTier::Moderate,
                    Category::Logs,
                )
                .with_note("system logs — requires root"),
            );
        }
    }

    items.sort_by(|a, b| b.real_bytes.cmp(&a.real_bytes));
    Ok(items)
}

fn dir_max_atime_days(path: &std::path::Path) -> u32 {
    let mut max_days = 0u32;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        if let Ok(md) = entry.metadata() {
            max_days = max_days.max(atime_days(&md));
        }
    }
    max_days
}
