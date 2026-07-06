//! iCloud scanner: find locally-materialized files that can be evicted.
//!
//! `brctl evict` works on individual iCloud documents, not binaries
//! inside `.app` bundles, `node_modules`, or framework `Versions/` trees.

use crate::fs_util::{home_dir, inode_id, real_size};
use crate::model::{Category, ItemAction, SafetyTier, ScanItem};
use crate::scan::ScanContext;
use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

const MIN_BYTES: u64 = 10 * 1024 * 1024;
const MAX_DEPTH: usize = 24;

/// Directory names skipped during traversal.
const SKIP_DIR_NAMES: &[&str] = &[
    "node_modules",
    ".git",
    "__pycache__",
    "target",
    ".npm",
    "Pods",
    "DerivedData",
    "build",
    ".venv",
    "venv",
    ".tox",
    ".gradle",
    "out",
];

pub fn scan(_ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let mut items = Vec::new();
    let icloud_root = home_dir().join("Library/Mobile Documents/com~apple~CloudDocs");

    if !icloud_root.is_dir() {
        return Ok(items);
    }

    let mut seen_inodes = HashSet::new();
    walk_icloud(&icloud_root, &icloud_root, &mut items, &mut seen_inodes, 0);

    items.sort_by(|a, b| b.real_bytes.cmp(&a.real_bytes));
    Ok(items)
}

fn walk_icloud(
    root: &Path,
    dir: &Path,
    items: &mut Vec<ScanItem>,
    seen_inodes: &mut HashSet<(u64, u64)>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') {
            continue;
        }

        if name.ends_with(".icloud") {
            continue;
        }

        let Ok(md) = std::fs::symlink_metadata(&path) else {
            continue;
        };

        if md.file_type().is_symlink() {
            continue;
        }

        if md.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            walk_icloud(root, &path, items, seen_inodes, depth + 1);
            continue;
        }

        if !md.is_file() {
            continue;
        }

        if should_skip_file_path(&path, root) {
            continue;
        }

        let bytes = real_size(&md);
        if bytes < MIN_BYTES {
            continue;
        }

        let placeholder = path.with_file_name(format!(".{name}.icloud"));
        if placeholder.exists() {
            continue;
        }

        let id = inode_id(&md);
        if !seen_inodes.insert(id) {
            continue;
        }

        let label = display_label(root, &path);
        items.push(
            ScanItem::new(path, label, bytes, SafetyTier::Moderate, Category::ICloud)
                .with_note("evict local copy — stays in iCloud")
                .with_action(ItemAction::Evict),
        );
    }
}

fn should_skip_dir(name: &str) -> bool {
    SKIP_DIR_NAMES.contains(&name)
        || name.ends_with(".app")
        || name.ends_with(".framework")
        || name.ends_with(".bundle")
        || name.ends_with(".plugin")
        || name.ends_with(".xcodeproj")
        || name.ends_with(".xcworkspace")
}

fn should_skip_file_path(path: &Path, root: &Path) -> bool {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    if rel.contains("/node_modules/")
        || rel.contains("/Contents/")
        || rel.contains("/Versions/")
        || rel.contains("/Frameworks/")
    {
        return true;
    }

    for part in rel.split('/') {
        if part.ends_with(".app") || part.ends_with(".framework") {
            return true;
        }
    }

    false
}

fn display_label(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn skips_dev_and_bundle_directories() {
        assert!(should_skip_dir("node_modules"));
        assert!(should_skip_dir("Electron.app"));
        assert!(should_skip_dir("Electron Framework.framework"));
        assert!(!should_skip_dir("Documents"));
    }

    #[test]
    fn skips_app_internals_and_node_modules_files() {
        let root = PathBuf::from("/icloud");
        let electron = root.join(
            "Documents/Mac-Standalone/node_modules/electron/dist/Electron.app/Contents/Frameworks/Electron Framework.framework/Versions/A/Electron Framework",
        );
        assert!(should_skip_file_path(&electron, &root));

        let pdf = root.join("homework4.pdf");
        assert!(!should_skip_file_path(&pdf, &root));

        let nested = root.join("Documents/report.pdf");
        assert!(!should_skip_file_path(&nested, &root));
    }

    #[test]
    fn display_label_is_relative_to_icloud_root() {
        let root = PathBuf::from("/icloud");
        let path = root.join("Documents/homework4.pdf");
        assert_eq!(display_label(&root, &path), "Documents/homework4.pdf");
    }
}
