//! Trash scanner.

use crate::fs_util::user_trash_dir;
use crate::model::{Category, ItemAction, SafetyTier, ScanItem};
use crate::scan::{ScanContext, label_for, path_bytes};
use anyhow::Result;

pub fn scan(_ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let mut items = Vec::new();
    let trash = user_trash_dir();

    if !trash.is_dir() {
        return Ok(items);
    }

    let total_bytes = path_bytes(&trash);
    if total_bytes > 0 {
        items.push(
            ScanItem::new(
                trash.clone(),
                "Empty entire Trash",
                total_bytes,
                SafetyTier::Safe,
                Category::Trash,
            )
            .with_note("permanently delete everything in Trash")
            .with_action(ItemAction::EmptyTrash),
        );
    }

    if let Ok(entries) = std::fs::read_dir(&trash) {
        for entry in entries.flatten() {
            let path = entry.path();
            let bytes = path_bytes(&path);
            if bytes == 0 {
                continue;
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "trashed item".into());
            let label = label_for(&path, &name);
            items.push(
                ScanItem::new(path, label, bytes, SafetyTier::Safe, Category::Trash)
                    .with_note("permanent delete — already in Trash")
                    .with_action(ItemAction::Delete),
            );
        }
    }

    items.sort_by(|a, b| b.real_bytes.cmp(&a.real_bytes));
    Ok(items)
}
