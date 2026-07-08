//! Developer artifact scanner.

use crate::fs_util::home_dir;
use crate::model::{Category, SafetyTier, ScanItem};
use crate::scan::{ScanContext, path_bytes, walk_parallel};
use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub fn scan(ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let items_mtx = Mutex::new(Vec::<ScanItem>::new());
    let seen = Mutex::new(HashSet::<PathBuf>::new());

    for root in ctx.config.dev_artifact_roots() {
        let matchers = &ctx.matchers;
        walk_parallel(
            &root,
            matchers,
            |path, name| {
                if should_skip_dir(ctx, path) {
                    return true;
                }
                if is_artifact_dir(ctx, name) {
                    push_candidate(path, "generated build artifact", &items_mtx, &seen);
                    return true;
                }
                if is_dependency_dir(ctx, name) {
                    push_candidate(
                        path,
                        "dependency folder — reinstall may be needed",
                        &items_mtx,
                        &seen,
                    );
                    return true;
                }
                false
            },
            |_path, _name| {},
        );
    }

    add_known_developer_locations(ctx, &items_mtx, &seen);
    let mut items = items_mtx.into_inner().unwrap();
    items.sort_by(|a, b| b.real_bytes.cmp(&a.real_bytes));
    Ok(items)
}

fn add_known_developer_locations(
    ctx: &ScanContext,
    items: &Mutex<Vec<ScanItem>>,
    seen: &Mutex<HashSet<PathBuf>>,
) {
    for path in ctx.config.dev_artifact_review_roots() {
        if !ctx.matchers.is_protected(&path) {
            push_candidate(&path, review_root_note(&path), items, seen);
        }
    }
}

fn review_root_note(path: &Path) -> &'static str {
    let path = path.to_string_lossy();
    if path.contains("DerivedData") {
        "Xcode DerivedData — rebuildable project cache"
    } else if path.contains("Archives") {
        "Xcode archives — review before deleting"
    } else if path.contains("DeviceSupport") {
        "Xcode device support files — review before deleting"
    } else if path.contains("CoreSimulator") {
        "CoreSimulator data — review before deleting"
    } else {
        "developer tool artifact — review before deleting"
    }
}

fn is_artifact_dir(ctx: &ScanContext, name: &str) -> bool {
    ctx.config
        .dev_artifacts
        .artifact_dir_names
        .iter()
        .any(|n| n == name)
}

fn is_dependency_dir(ctx: &ScanContext, name: &str) -> bool {
    ctx.config
        .dev_artifacts
        .dependency_dir_names
        .iter()
        .any(|n| n == name)
}

fn should_skip_dir(ctx: &ScanContext, path: &Path) -> bool {
    if ctx.matchers.is_protected(path) {
        return true;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name.ends_with(".app")
        || name.ends_with(".framework")
        || name.ends_with(".bundle")
        || name.ends_with(".photoslibrary")
}

fn push_candidate(
    path: &Path,
    note: &'static str,
    items: &Mutex<Vec<ScanItem>>,
    seen: &Mutex<HashSet<PathBuf>>,
) {
    if !path.is_dir() {
        return;
    }
    let key = path.to_path_buf();
    {
        let mut guard = seen.lock().unwrap();
        if !guard.insert(key) {
            return;
        }
    }
    let bytes = path_bytes(path);
    if bytes == 0 {
        return;
    }
    items.lock().unwrap().push(
        ScanItem::new(
            path.to_path_buf(),
            display_label(path),
            bytes,
            SafetyTier::Moderate,
            Category::DevArtifacts,
        )
        .with_note(note),
    );
}

fn display_label(path: &Path) -> String {
    let home = home_dir();
    path.strip_prefix(&home)
        .unwrap_or(path)
        .display()
        .to_string()
}
