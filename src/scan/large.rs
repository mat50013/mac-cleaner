//! Large-file scanner.

use crate::fs_util::{atime_days, real_size};
use crate::model::{Category, SafetyTier, ScanItem};
use crate::scan::{ScanContext, label_for, walk_parallel};
use anyhow::Result;
use std::path::Path;
use std::sync::Mutex;

pub fn scan(ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let min = ctx.config.large.min_bytes;
    let stale_min = ctx.config.large.stale_archive_min_bytes;
    let stale_days = ctx.config.large.stale_archive_days;
    let items_mtx = Mutex::new(Vec::<ScanItem>::new());

    for root in ctx.config.large_roots() {
        let matchers = &ctx.matchers;
        walk_parallel(
            &root,
            matchers,
            |_path, _name| false,
            |path, _name| {
                let Ok(md) = std::fs::symlink_metadata(path) else {
                    return;
                };
                if !md.is_file() {
                    return;
                }
                let bytes = real_size(&md);
                let days = atime_days(&md);
                let stale_archive =
                    is_installer_or_archive(path) && bytes >= stale_min && days >= stale_days;
                if bytes < min && !stale_archive {
                    return;
                }
                let note = if stale_archive {
                    "stale installer/archive — review before deleting"
                } else {
                    "user file — review before deleting"
                };
                items_mtx.lock().unwrap().push(
                    ScanItem::new(
                        path.to_path_buf(),
                        label_for(path, "large file"),
                        bytes,
                        SafetyTier::Moderate,
                        Category::LargeFiles,
                    )
                    .with_age(days)
                    .with_note(note),
                );
            },
        );
    }

    let mut items = items_mtx.into_inner().unwrap();

    items.sort_by(|a, b| b.real_bytes.cmp(&a.real_bytes));
    Ok(items)
}

fn is_installer_or_archive(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(
        name.as_str(),
        n if n.ends_with(".dmg")
            || n.ends_with(".pkg")
            || n.ends_with(".zip")
            || n.ends_with(".tar")
            || n.ends_with(".tar.gz")
            || n.ends_with(".tgz")
            || n.ends_with(".iso")
            || n.ends_with(".run")
    )
}
