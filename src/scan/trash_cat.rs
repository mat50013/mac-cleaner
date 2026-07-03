//! Trash-bin scanner: report size and offer to empty.

use crate::fs_util::home_dir;
use crate::model::{Category, ItemAction, SafetyTier, ScanItem};
use crate::scan::{path_bytes, ScanContext};
use anyhow::Result;

pub fn scan(_ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let mut items = Vec::new();
    let trash = home_dir().join(".Trash");

    if trash.is_dir() {
        let bytes = path_bytes(&trash);
        if bytes > 0 {
            items.push(
                ScanItem::new(
                    trash,
                    "User Trash",
                    bytes,
                    SafetyTier::Safe,
                    Category::Trash,
                )
                .with_note("already in Trash — empty permanently")
                .with_action(ItemAction::EmptyTrash),
            );
        }
    }

    Ok(items)
}
