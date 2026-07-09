use mac_cleaner::config::Config;
use mac_cleaner::model::{Category, ItemAction, SafetyTier};
use tempfile::tempdir;

use crate::common::ctx_for;

#[test]
fn given_icloud_tree_when_scan_then_filters_placeholders_and_sets_evict_action() {
    let home = tempdir().expect("tempdir");
    let home_path = home.path().to_path_buf();
    // SAFETY: tests are single-process and this test restores HOME before exit.
    let old_home = std::env::var_os("HOME");
    unsafe { std::env::set_var("HOME", &home_path) };

    let run = || {
        let root = home_path.join("Library/Mobile Documents/com~apple~CloudDocs");
        std::fs::create_dir_all(&root).expect("create icloud root");

        let big = root.join("Docs/big.mov");
        std::fs::create_dir_all(big.parent().expect("big parent")).expect("mkdirs");
        // Write enough blocks to pass MIN_BYTES (10 MiB).
        std::fs::write(&big, vec![3u8; 11 * 1024 * 1024]).expect("write big");

        // Placeholder companion means skip.
        let with_placeholder = root.join("Docs/placeholder.pdf");
        std::fs::write(&with_placeholder, vec![1u8; 11 * 1024 * 1024]).expect("write placeholder");
        std::fs::write(root.join("Docs/.placeholder.pdf.icloud"), b"").expect("write .icloud");

        // Bundle internals should be skipped.
        let app_internal = root.join("App.app/Contents/Frameworks/lib.bin");
        std::fs::create_dir_all(app_internal.parent().expect("internal parent")).expect("mkdirs");
        std::fs::write(&app_internal, vec![2u8; 11 * 1024 * 1024]).expect("write app internal");

        let cfg = Config::default();
        let items =
            mac_cleaner::scan::icloud::scan(&ctx_for(&cfg, Category::ICloud)).expect("scan");

        assert_eq!(
            items.len(),
            1,
            "expected only one evictable file: {items:?}"
        );
        let item = &items[0];
        assert!(item.path.ends_with("big.mov"));
        assert_eq!(item.category, Category::ICloud);
        assert_eq!(item.tier, SafetyTier::Moderate);
        assert_eq!(item.action, ItemAction::Evict);
        assert_eq!(item.regen_note, "evict local copy — stays in iCloud");
    };

    run();
    match old_home {
        Some(v) => unsafe { std::env::set_var("HOME", v) },
        None => unsafe { std::env::remove_var("HOME") },
    }
}
