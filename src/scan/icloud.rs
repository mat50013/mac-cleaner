//! iCloud scanner: find locally-materialized files that can be evicted.

use crate::fs_util::{home_dir, real_size};
use crate::model::{Category, ItemAction, SafetyTier, ScanItem};
use crate::scan::ScanContext;
use anyhow::Result;

pub fn scan(_ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let mut items = Vec::new();
    let icloud_root = home_dir().join("Library/Mobile Documents/com~apple~CloudDocs");

    if !icloud_root.is_dir() {
        return Ok(items);
    }

    walk_icloud(&icloud_root, &mut items, 0);

    items.sort_by(|a, b| b.real_bytes.cmp(&a.real_bytes));
    Ok(items)
}

fn walk_icloud(dir: &std::path::Path, items: &mut Vec<ScanItem>, depth: usize) {
    if depth > 12 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip iCloud placeholder files (cloud-only, not downloaded).
        if name.ends_with(".icloud") {
            continue;
        }

        if path.is_dir() {
            walk_icloud(&path, items, depth + 1);
            continue;
        }

        let Ok(md) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !md.is_file() {
            continue;
        }
        let bytes = real_size(&md);
        if bytes < 10 * 1024 * 1024 {
            continue; // only surface files >= 10 MB
        }

        // A materialized file has no sibling .icloud placeholder.
        let placeholder = path.with_file_name(format!(".{name}.icloud"));
        if placeholder.exists() {
            continue; // not fully synced yet
        }

        items.push(
            ScanItem::new(
                path.clone(),
                name,
                bytes,
                SafetyTier::Moderate,
                Category::ICloud,
            )
            .with_note("evict local copy — stays in iCloud")
            .with_action(ItemAction::Evict),
        );
    }
}
