//! Large-file scanner.

use crate::fs_util::{atime_days, real_size};
use crate::model::{Category, SafetyTier, ScanItem};
use crate::scan::{ScanContext, label_for, walk_parallel};
use anyhow::Result;
use std::sync::Mutex;

pub fn scan(ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let min = ctx.config.large.min_bytes;
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
                if bytes < min {
                    return;
                }
                items_mtx.lock().unwrap().push(
                    ScanItem::new(
                        path.to_path_buf(),
                        label_for(path, "large file"),
                        bytes,
                        SafetyTier::Moderate,
                        Category::LargeFiles,
                    )
                    .with_age(atime_days(&md))
                    .with_note("user file — review before deleting"),
                );
            },
        );
    }

    let mut items = items_mtx.into_inner().unwrap();

    items.sort_by(|a, b| b.real_bytes.cmp(&a.real_bytes));
    Ok(items)
}
