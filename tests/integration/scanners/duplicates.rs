use mac_cleaner::config::Config;
use mac_cleaner::model::Category;
use tempfile::tempdir;

use crate::common::{ctx_for, path_str, write_file};

#[test]
fn given_dup_pair_when_scan_then_groups_and_locks_one_keeper() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let shared = vec![7u8; 8192];
    write_file(&root.join("a.bin"), &shared);
    write_file(&root.join("b.bin"), &shared);
    let mut other = vec![7u8; 8192];
    other[0] = 42;
    write_file(&root.join("c.bin"), &other);
    write_file(&root.join("tiny.bin"), &[7u8; 16]);

    let mut cfg = Config::default();
    cfg.duplicates.roots = vec![path_str(root)];
    cfg.duplicates.min_bytes = 1024;

    let items = mac_cleaner::scan::duplicates::scan(&ctx_for(&cfg, Category::Duplicates))
        .expect("scan duplicates");

    assert_eq!(items.len(), 2, "one dup pair expected: {items:?}");
    assert_eq!(items.iter().filter(|i| i.is_keeper).count(), 1);
    assert_eq!(items.iter().filter(|i| i.selected).count(), 1);
    assert!(items.iter().all(|i| !i.path.ends_with("c.bin")));
    assert_eq!(items[0].group_id, items[1].group_id);
    assert!(items[0].group_id.is_some());
}

#[test]
fn given_three_identical_when_scan_then_one_keeper_two_selected() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let shared = vec![9u8; 300_000];
    write_file(&root.join("a.bin"), &shared);
    write_file(&root.join("b.bin"), &shared);
    write_file(&root.join("c.bin"), &shared);

    let mut cfg = Config::default();
    cfg.duplicates.roots = vec![path_str(root)];
    cfg.duplicates.min_bytes = 1024;

    let items = mac_cleaner::scan::duplicates::scan(&ctx_for(&cfg, Category::Duplicates))
        .expect("scan duplicates");

    assert_eq!(items.len(), 3);
    assert_eq!(items.iter().filter(|i| i.is_keeper).count(), 1);
    assert_eq!(items.iter().filter(|i| i.selected).count(), 2);
    let gid = items[0].group_id.expect("group id");
    assert!(items.iter().all(|i| i.group_id == Some(gid)));
    assert!(items.iter().all(|i| i.selected != i.is_keeper));
}

#[test]
fn given_min_threshold_when_files_below_it_then_ignored() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let shared = vec![3u8; 200_000];
    write_file(&root.join("a.bin"), &shared);
    write_file(&root.join("b.bin"), &shared);

    let mut cfg = Config::default();
    cfg.duplicates.roots = vec![path_str(root)];
    cfg.duplicates.min_bytes = 1_000_000;

    let items = mac_cleaner::scan::duplicates::scan(&ctx_for(&cfg, Category::Duplicates))
        .expect("scan duplicates");
    assert!(items.is_empty(), "sub-threshold files must be ignored");
}

#[test]
fn given_same_size_diff_bytes_when_scan_then_not_grouped() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    write_file(&root.join("a.bin"), &vec![1u8; 200_000]);
    write_file(&root.join("b.bin"), &vec![2u8; 200_000]);

    let mut cfg = Config::default();
    cfg.duplicates.roots = vec![path_str(root)];
    cfg.duplicates.min_bytes = 1024;

    let items = mac_cleaner::scan::duplicates::scan(&ctx_for(&cfg, Category::Duplicates))
        .expect("scan duplicates");
    assert!(
        items.is_empty(),
        "same size but different bytes are not duplicates"
    );
}
