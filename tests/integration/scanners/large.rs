use mac_cleaner::config::Config;
use mac_cleaner::model::{Category, SafetyTier};
use tempfile::tempdir;

use crate::common::{ctx_for, path_str, write_file};

#[test]
fn given_threshold_when_scan_large_then_only_big_items_return() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("big.bin"), &vec![0u8; 2_000_000]);
    write_file(&root.join("small.bin"), &[0u8; 1000]);

    let mut cfg = Config::default();
    cfg.large.roots = vec![path_str(root)];
    cfg.large.min_bytes = 500_000;

    let items =
        mac_cleaner::scan::large::scan(&ctx_for(&cfg, Category::LargeFiles)).expect("scan large");

    assert_eq!(items.len(), 1);
    assert!(items[0].path.ends_with("big.bin"));
    assert!(items[0].real_bytes >= 2_000_000);
}

#[test]
fn given_excluded_dirs_when_scan_large_then_they_are_pruned() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("keep/big1.bin"), &vec![0u8; 2_000_000]);
    write_file(&root.join("keep/big2.bin"), &vec![0u8; 3_000_000]);
    write_file(&root.join("node_modules/huge.bin"), &vec![0u8; 5_000_000]);
    write_file(&root.join("small.bin"), &[0u8; 1000]);

    let mut cfg = Config::default();
    cfg.large.roots = vec![path_str(root)];
    cfg.large.min_bytes = 500_000;

    let items =
        mac_cleaner::scan::large::scan(&ctx_for(&cfg, Category::LargeFiles)).expect("scan large");
    assert_eq!(items.len(), 2);
    assert!(items[0].real_bytes >= items[1].real_bytes);
    assert!(items[0].path.ends_with("big2.bin"));
    assert!(items[1].path.ends_with("big1.bin"));
    assert!(
        items
            .iter()
            .all(|i| !i.path.to_string_lossy().contains("node_modules"))
    );
}

#[test]
fn given_stale_archive_rule_when_below_large_threshold_then_still_included() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("old-tool.dmg"), &[0u8; 8192]);
    write_file(&root.join("notes.txt"), &[0u8; 8192]);

    let mut cfg = Config::default();
    cfg.large.roots = vec![path_str(root)];
    cfg.large.min_bytes = 1_000_000;
    cfg.large.stale_archive_min_bytes = 1;
    cfg.large.stale_archive_days = 0;

    let items =
        mac_cleaner::scan::large::scan(&ctx_for(&cfg, Category::LargeFiles)).expect("scan large");

    assert_eq!(items.len(), 1);
    assert!(items[0].path.ends_with("old-tool.dmg"));
    assert_eq!(items[0].tier, SafetyTier::Moderate);
    assert_eq!(
        items[0].regen_note,
        "stale installer/archive — review before deleting"
    );
}
