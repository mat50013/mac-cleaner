use mac_cleaner::config::Config;
use mac_cleaner::model::{Category, ItemAction};
use tempfile::tempdir;

use crate::common::{ctx_for, path_str, write_file};

#[test]
fn given_log_files_and_dirs_when_scan_logs_then_both_are_returned() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("app.log"), &[0u8; 4096]);
    write_file(&root.join("logs/service.out"), &[0u8; 4096]);

    let mut cfg = Config::default();
    cfg.logs.roots = vec![path_str(root)];

    let items = mac_cleaner::scan::logs::scan(&ctx_for(&cfg, Category::Logs)).expect("scan logs");

    assert!(items.iter().any(|i| i.path.ends_with("app.log")));
    assert!(items.iter().any(|i| i.path.ends_with("logs")));
}

#[test]
fn given_truncate_active_when_recent_log_then_action_is_truncate() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("app.log"), &[0u8; 8192]);

    let mut cfg = Config::default();
    cfg.logs.roots = vec![path_str(root)];
    cfg.logs.truncate_active = true;

    let items = mac_cleaner::scan::logs::scan(&ctx_for(&cfg, Category::Logs)).expect("scan logs");
    let app = items
        .iter()
        .find(|i| i.path.ends_with("app.log"))
        .expect("app.log found");
    assert_eq!(app.action, ItemAction::Truncate);
}

#[test]
fn given_truncate_disabled_when_recent_log_then_action_is_delete() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("app.log"), &[0u8; 8192]);

    let mut cfg = Config::default();
    cfg.logs.roots = vec![path_str(root)];
    cfg.logs.truncate_active = false;

    let items = mac_cleaner::scan::logs::scan(&ctx_for(&cfg, Category::Logs)).expect("scan logs");
    let app = items
        .iter()
        .find(|i| i.path.ends_with("app.log"))
        .expect("app.log found");
    assert_eq!(app.action, ItemAction::Delete);
}

#[test]
fn given_rotated_log_when_scan_then_matches_pattern() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("service.log.3"), &[0u8; 4096]);

    let mut cfg = Config::default();
    cfg.logs.roots = vec![path_str(root)];

    let items = mac_cleaner::scan::logs::scan(&ctx_for(&cfg, Category::Logs)).expect("scan logs");
    assert!(items.iter().any(|i| i.path.ends_with("service.log.3")));
}
