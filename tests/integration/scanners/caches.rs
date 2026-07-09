use mac_cleaner::config::Config;
use mac_cleaner::model::{Category, ItemAction, SafetyTier};
use tempfile::tempdir;

use crate::common::{ctx_for, path_str, write_file};

#[test]
fn given_cache_signatures_when_scan_caches_then_detects_and_classifies_tier() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Signature paths.
    write_file(&root.join("Cache/data.bin"), &[0u8; 4096]);
    write_file(&root.join("DerivedData/index.bin"), &[0u8; 4096]);
    write_file(&root.join("ShipIt/stale.bin"), &[0u8; 4096]);
    // Should be excluded by matcher rules.
    write_file(&root.join("node_modules/Cache/ignored.bin"), &[0u8; 4096]);
    write_file(&root.join("Chrome/Default/Cache/ignored.bin"), &[0u8; 4096]);

    let mut cfg = Config::default();
    cfg.cache.roots = vec![path_str(root)];
    cfg.cache.signatures = vec!["Cache".into(), "DerivedData".into(), "ShipIt".into()];

    let items =
        mac_cleaner::scan::caches::scan(&ctx_for(&cfg, Category::Caches)).expect("scan caches");

    assert!(items.iter().any(|i| i.path.ends_with("Cache")));
    assert!(
        items
            .iter()
            .any(|i| i.path.ends_with("DerivedData") && i.tier == SafetyTier::Moderate)
    );
    assert!(
        items
            .iter()
            .any(|i| i.path.ends_with("ShipIt") && i.tier == SafetyTier::Safe)
    );
    assert!(
        items
            .iter()
            .all(|i| !i.path.to_string_lossy().contains("node_modules/Cache"))
    );
    assert!(
        items
            .iter()
            .all(|i| !i.path.to_string_lossy().contains("Default/Cache"))
    );
}

#[test]
fn given_duplicate_roots_when_scan_then_cache_dir_emitted_once() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("Cache/data.bin"), &[0u8; 8192]);

    let mut cfg = Config::default();
    cfg.cache.roots = vec![path_str(root), path_str(root)];
    cfg.cache.signatures = vec!["Cache".into()];

    let items =
        mac_cleaner::scan::caches::scan(&ctx_for(&cfg, Category::Caches)).expect("scan caches");

    let cache_count = items.iter().filter(|i| i.path.ends_with("Cache")).count();
    assert_eq!(cache_count, 1, "expected deduped cache dir");
}

#[test]
fn given_cache_items_when_ingest_then_safe_items_autoselect_and_actions_are_delete() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("Cache/data.bin"), &[0u8; 4096]);

    let mut cfg = Config::default();
    cfg.cache.roots = vec![path_str(root)];
    cfg.cache.signatures = vec!["Cache".into()];

    let items =
        mac_cleaner::scan::caches::scan(&ctx_for(&cfg, Category::Caches)).expect("scan caches");
    assert!(items.iter().all(|i| i.category == Category::Caches));
    assert!(items.iter().all(|i| i.action == ItemAction::Delete));
    assert!(items.iter().any(|i| i.tier == SafetyTier::Safe));
}
