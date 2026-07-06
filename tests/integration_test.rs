//! Integration tests for scanners and cleaning.

use clap::Parser;
use mac_cleaner::clean::{CleanOptions, run_clean};
use mac_cleaner::cli::{Cli, Commands};
use mac_cleaner::config::{Config, DeleteMode};
use mac_cleaner::event::{Event, WorkerMsg, WorkerSender};
use mac_cleaner::fs_util::{dir_real_size, expand_tilde, human_size};
use mac_cleaner::model::{Category, ItemAction, SafetyTier, ScanItem};
use mac_cleaner::scan::{self, ScanContext};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut f = fs::File::create(path).unwrap();
    f.write_all(bytes).unwrap();
    f.flush().unwrap();
}

fn ctx_for(cfg: &Config, cat: Category) -> ScanContext {
    ScanContext {
        config: Arc::new(cfg.clone()),
        matchers: cfg.matchers().unwrap(),
        tx: WorkerSender::null(),
        categories: vec![cat],
    }
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

fn clean_and_wait(items: Vec<ScanItem>, opts: CleanOptions) -> (u64, Vec<String>) {
    let (worker, rx) = WorkerSender::channel();
    run_clean(items, opts, worker);
    loop {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Worker(WorkerMsg::CleanDone { freed, failures })) => {
                return (freed, failures);
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out waiting for CleanDone"),
        }
    }
}

/// Like `clean_and_wait`, but also captures the `(done, total)` progress ticks.
fn clean_collect(items: Vec<ScanItem>, opts: CleanOptions) -> (Vec<(usize, usize)>, u64) {
    let (worker, rx) = WorkerSender::channel();
    run_clean(items, opts, worker);
    let mut progress = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Worker(WorkerMsg::CleanProgress { done, total, .. })) => {
                progress.push((done, total));
            }
            Ok(Event::Worker(WorkerMsg::CleanDone { freed, .. })) => {
                return (progress, freed);
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out waiting for CleanDone"),
        }
    }
}

#[test]
fn human_size_formats() {
    assert_eq!(human_size(1024 * 1024), "1.0 MB");
    assert_eq!(human_size(0), "0 B");
}

#[test]
fn expand_tilde_works() {
    let home = std::env::var("HOME").unwrap();
    assert_eq!(expand_tilde("~").to_string_lossy(), home);
    assert_eq!(
        expand_tilde("~/Downloads").to_string_lossy(),
        format!("{home}/Downloads")
    );
}

#[test]
fn config_signatures() {
    let m = Config::default().matchers().unwrap();
    assert!(m.is_cache_signature("GPUCache"));
    assert!(m.is_log_dir("logs"));
    assert!(!m.is_protected(Path::new("/Users/x/Library/Caches/foo")));
    assert!(m.is_protected(Path::new("/Users/x/Chrome/Default")));
    assert!(m.is_excluded_dir("node_modules"));
}

#[test]
fn dir_real_size_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a.bin");
    write_file(&path, &[0u8; 4096]);
    let mut seen = HashSet::new();
    assert!(dir_real_size(&path, &mut seen) >= 4096);
}

#[test]
fn dir_real_size_recurses_and_dedups() {
    let dir = tempdir().unwrap();
    write_file(&dir.path().join("a/one.bin"), &[1u8; 8192]);
    write_file(&dir.path().join("a/b/two.bin"), &[2u8; 8192]);
    let mut seen = HashSet::new();
    let total = dir_real_size(dir.path(), &mut seen);
    assert!(total >= 16384, "expected >= 16384, got {total}");
}

#[test]
fn walk_parallel_prunes_excluded_and_protected() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_file(&root.join("Cache/data.bin"), &[0u8; 2048]);
    write_file(&root.join("node_modules/Cache/data.bin"), &[0u8; 2048]);
    write_file(&root.join("Default/Cache/data.bin"), &[0u8; 2048]);
    write_file(&root.join("normal/file.txt"), &[0u8; 16]);

    let m = Config::default().matchers().unwrap();
    let found = Mutex::new(Vec::<PathBuf>::new());
    scan::walk_parallel(
        root,
        &m,
        |path, name| {
            if m.is_cache_signature(name) {
                found.lock().unwrap().push(path.to_path_buf());
                true
            } else {
                false
            }
        },
        |_p, _n| {},
    );

    let found = found.into_inner().unwrap();
    assert_eq!(found.len(), 1, "found: {found:?}");
    assert!(found[0].ends_with("Cache"));
}

#[test]
fn duplicates_scan_groups_identical_and_keeps_one() {
    let dir = tempdir().unwrap();
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

    let items = scan::duplicates::scan(&ctx_for(&cfg, Category::Duplicates)).unwrap();

    assert_eq!(items.len(), 2, "one dup pair expected: {items:?}");
    assert_eq!(items.iter().filter(|i| i.is_keeper).count(), 1);
    let selected: Vec<_> = items.iter().filter(|i| i.selected).collect();
    assert_eq!(selected.len(), 1);
    assert!(!selected[0].is_keeper);
    assert_eq!(items[0].group_id, items[1].group_id);
    assert!(items[0].group_id.is_some());
    assert!(items.iter().all(|i| !i.path.ends_with("c.bin")));
}

#[test]
fn large_scan_applies_threshold() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_file(&root.join("big.bin"), &vec![0u8; 2_000_000]);
    write_file(&root.join("small.bin"), &[0u8; 1000]);

    let mut cfg = Config::default();
    cfg.large.roots = vec![path_str(root)];
    cfg.large.min_bytes = 500_000;

    let items = scan::large::scan(&ctx_for(&cfg, Category::LargeFiles)).unwrap();

    assert_eq!(items.len(), 1, "only the big file: {items:?}");
    assert!(items[0].path.ends_with("big.bin"));
    assert!(items[0].real_bytes >= 2_000_000);
}

#[test]
fn logs_scan_finds_dirs_and_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_file(&root.join("app.log"), &[0u8; 4096]);
    write_file(&root.join("logs/service.out"), &[0u8; 4096]);

    let mut cfg = Config::default();
    cfg.logs.roots = vec![path_str(root)];

    let items = scan::logs::scan(&ctx_for(&cfg, Category::Logs)).unwrap();

    assert!(
        items.iter().any(|i| i.path.ends_with("app.log")),
        "app.log missing: {items:?}"
    );
    assert!(
        items.iter().any(|i| i.path.ends_with("logs")),
        "logs dir missing: {items:?}"
    );
}

#[test]
fn clean_dry_run_deletes_nothing_but_reports_freed() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("f.bin");
    write_file(&p, &[0u8; 4096]);
    let item = ScanItem::new(p.clone(), "f", 4096, SafetyTier::Safe, Category::Caches);

    let (freed, failures) = clean_and_wait(
        vec![item],
        CleanOptions {
            permanent: false,
            dry_run: true,
            mode: DeleteMode::Trash,
        },
    );

    assert!(p.exists(), "dry run must not delete");
    assert_eq!(freed, 4096);
    assert!(failures.is_empty());
}

#[test]
fn clean_permanent_removes_file() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("gone.bin");
    write_file(&p, &[0u8; 4096]);
    let item = ScanItem::new(p.clone(), "gone", 4096, SafetyTier::Safe, Category::Caches);

    let (freed, failures) = clean_and_wait(
        vec![item],
        CleanOptions {
            permanent: true,
            dry_run: false,
            mode: DeleteMode::Permanent,
        },
    );

    assert!(!p.exists(), "permanent delete must remove the file");
    assert_eq!(freed, 4096);
    assert!(failures.is_empty());
}

#[test]
fn clean_missing_path_is_a_noop_not_an_error() {
    let item = ScanItem::new(
        PathBuf::from("/definitely/not/here/xyz.bin"),
        "ghost",
        4096,
        SafetyTier::Safe,
        Category::Caches,
    );

    let (freed, failures) = clean_and_wait(
        vec![item],
        CleanOptions {
            permanent: true,
            dry_run: false,
            mode: DeleteMode::Permanent,
        },
    );

    assert_eq!(freed, 0, "missing path frees nothing");
    assert!(failures.is_empty(), "missing path is not a failure");
}

#[test]
fn clean_truncate_empties_file_in_place() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("active.log");
    write_file(&p, &[0u8; 10_000]);
    let mut item = ScanItem::new(
        p.clone(),
        "active.log",
        10_000,
        SafetyTier::Safe,
        Category::Logs,
    );
    item.action = ItemAction::Truncate;

    let (_freed, failures) = clean_and_wait(
        vec![item],
        CleanOptions {
            permanent: false,
            dry_run: false,
            mode: DeleteMode::Trash,
        },
    );

    assert!(failures.is_empty(), "truncate failed: {failures:?}");
    assert!(p.exists(), "truncate keeps the file");
    assert_eq!(fs::metadata(&p).unwrap().len(), 0, "file should be emptied");
}

#[test]
fn duplicates_three_identical_files_share_one_group_and_one_keeper() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let shared = vec![9u8; 300_000];
    write_file(&root.join("a.bin"), &shared);
    write_file(&root.join("b.bin"), &shared);
    write_file(&root.join("c.bin"), &shared);

    let mut cfg = Config::default();
    cfg.duplicates.roots = vec![path_str(root)];
    cfg.duplicates.min_bytes = 1024;

    let items = scan::duplicates::scan(&ctx_for(&cfg, Category::Duplicates)).unwrap();

    assert_eq!(items.len(), 3, "all three copies reported: {items:?}");
    assert_eq!(items.iter().filter(|i| i.is_keeper).count(), 1);
    assert_eq!(items.iter().filter(|i| i.selected).count(), 2);

    let gid = items[0].group_id;
    assert!(gid.is_some());
    assert!(items.iter().all(|i| i.group_id == gid));
    assert!(items.iter().all(|i| i.selected != i.is_keeper));
}

#[test]
fn duplicates_respect_min_bytes_threshold() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let shared = vec![3u8; 200_000];
    write_file(&root.join("a.bin"), &shared);
    write_file(&root.join("b.bin"), &shared);

    let mut cfg = Config::default();
    cfg.duplicates.roots = vec![path_str(root)];
    cfg.duplicates.min_bytes = 1_000_000; // both files are far below this

    let items = scan::duplicates::scan(&ctx_for(&cfg, Category::Duplicates)).unwrap();
    assert!(
        items.is_empty(),
        "sub-threshold files must be ignored: {items:?}"
    );
}

#[test]
fn duplicates_same_size_different_content_not_grouped() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    write_file(&root.join("a.bin"), &vec![1u8; 200_000]);
    write_file(&root.join("b.bin"), &vec![2u8; 200_000]);

    let mut cfg = Config::default();
    cfg.duplicates.roots = vec![path_str(root)];
    cfg.duplicates.min_bytes = 1024;

    let items = scan::duplicates::scan(&ctx_for(&cfg, Category::Duplicates)).unwrap();
    assert!(
        items.is_empty(),
        "same size but different bytes are not duplicates: {items:?}"
    );
}

#[test]
fn large_scan_sorts_by_size_and_skips_excluded_dirs() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_file(&root.join("keep/big1.bin"), &vec![0u8; 2_000_000]);
    write_file(&root.join("keep/big2.bin"), &vec![0u8; 3_000_000]);
    write_file(&root.join("node_modules/huge.bin"), &vec![0u8; 5_000_000]);
    write_file(&root.join("small.bin"), &[0u8; 1000]);

    let mut cfg = Config::default();
    cfg.large.roots = vec![path_str(root)];
    cfg.large.min_bytes = 500_000;

    let items = scan::large::scan(&ctx_for(&cfg, Category::LargeFiles)).unwrap();

    assert_eq!(
        items.len(),
        2,
        "only the two big files under keep/: {items:?}"
    );
    assert!(
        items[0].real_bytes >= items[1].real_bytes,
        "sorted descending"
    );
    assert!(items[0].path.ends_with("big2.bin"));
    assert!(items[1].path.ends_with("big1.bin"));
    assert!(
        items
            .iter()
            .all(|i| !i.path.to_string_lossy().contains("node_modules")),
        "excluded dirs must be pruned: {items:?}"
    );
}

#[test]
fn logs_recent_file_gets_truncate_action_when_enabled() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_file(&root.join("app.log"), &[0u8; 8192]);

    let mut cfg = Config::default();
    cfg.logs.roots = vec![path_str(root)];
    cfg.logs.truncate_active = true;

    let items = scan::logs::scan(&ctx_for(&cfg, Category::Logs)).unwrap();
    let app = items
        .iter()
        .find(|i| i.path.ends_with("app.log"))
        .expect("app.log found");
    assert_eq!(
        app.action,
        ItemAction::Truncate,
        "a fresh active log should be truncated, not deleted"
    );
}

#[test]
fn logs_recent_file_is_deleted_when_truncation_disabled() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_file(&root.join("app.log"), &[0u8; 8192]);

    let mut cfg = Config::default();
    cfg.logs.roots = vec![path_str(root)];
    cfg.logs.truncate_active = false;

    let items = scan::logs::scan(&ctx_for(&cfg, Category::Logs)).unwrap();
    let app = items
        .iter()
        .find(|i| i.path.ends_with("app.log"))
        .expect("app.log found");
    assert_eq!(app.action, ItemAction::Delete);
}

#[test]
fn logs_scan_matches_rotated_log_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_file(&root.join("service.log.3"), &[0u8; 4096]);

    let mut cfg = Config::default();
    cfg.logs.roots = vec![path_str(root)];

    let items = scan::logs::scan(&ctx_for(&cfg, Category::Logs)).unwrap();
    assert!(
        items.iter().any(|i| i.path.ends_with("service.log.3")),
        "rotated log should match *.log.*: {items:?}"
    );
}

#[test]
fn config_loads_overrides_and_keeps_defaults() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    write_file(
        &path,
        b"delete_mode = \"permanent\"\n\n[large]\nmin_bytes = 12345\n",
    );

    let cfg = Config::load(Some(&path)).unwrap();
    assert_eq!(cfg.delete_mode, DeleteMode::Permanent);
    assert_eq!(cfg.large.min_bytes, 12345);
    // Unspecified sections fall back to defaults.
    assert_eq!(cfg.logs.age_days, 7);
    assert_eq!(cfg.duplicates.min_bytes, 1024 * 1024);
}

#[test]
fn config_missing_file_returns_defaults() {
    let cfg = Config::load(Some(Path::new("/no/such/mac-cleaner-config.toml"))).unwrap();
    assert_eq!(cfg.delete_mode, DeleteMode::Trash);
    assert_eq!(cfg.large.min_bytes, 100 * 1024 * 1024);
}

#[test]
fn cli_parse_categories_trims_and_drops_invalid() {
    let cats = Cli::parse_categories("caches, logs , nope, duplicates");
    assert_eq!(
        cats,
        vec![Category::Caches, Category::Logs, Category::Duplicates]
    );
    assert!(Cli::parse_categories("").is_empty());
    assert!(Cli::parse_categories("bogus,,").is_empty());
}

#[test]
fn cli_parses_scan_subcommand_and_global_flags() {
    let cli = Cli::try_parse_from(["mac-cleaner", "--dry-run", "scan", "--json"]).unwrap();
    assert!(cli.dry_run);
    match cli.command {
        Some(Commands::Scan { json, categories }) => {
            assert!(json);
            assert!(categories.is_none());
        }
        other => panic!("expected scan command, got {other:?}"),
    }
}

#[test]
fn cli_defaults_to_no_subcommand() {
    let cli = Cli::try_parse_from(["mac-cleaner"]).unwrap();
    assert!(cli.command.is_none());
    assert!(!cli.dry_run);
    assert!(!cli.no_elevate);
}

#[test]
fn clean_multiple_files_reports_total_freed() {
    let dir = tempdir().unwrap();
    let paths: Vec<_> = (0..3)
        .map(|i| dir.path().join(format!("f{i}.bin")))
        .collect();
    let mut items = Vec::new();
    for p in &paths {
        write_file(p, &[0u8; 4096]);
        items.push(ScanItem::new(
            p.clone(),
            "f",
            4096,
            SafetyTier::Safe,
            Category::Caches,
        ));
    }

    let (freed, failures) = clean_and_wait(
        items,
        CleanOptions {
            permanent: true,
            dry_run: false,
            mode: DeleteMode::Permanent,
        },
    );

    assert_eq!(freed, 4096 * 3);
    assert!(failures.is_empty());
    assert!(paths.iter().all(|p| !p.exists()), "all files removed");
}

#[test]
fn clean_mixes_present_and_missing_paths() {
    let dir = tempdir().unwrap();
    let present = dir.path().join("here.bin");
    write_file(&present, &[0u8; 4096]);

    let items = vec![
        ScanItem::new(
            present.clone(),
            "here",
            4096,
            SafetyTier::Safe,
            Category::Caches,
        ),
        ScanItem::new(
            PathBuf::from("/definitely/missing/ghost.bin"),
            "ghost",
            4096,
            SafetyTier::Safe,
            Category::Caches,
        ),
    ];

    let (freed, failures) = clean_and_wait(
        items,
        CleanOptions {
            permanent: true,
            dry_run: false,
            mode: DeleteMode::Permanent,
        },
    );

    assert_eq!(freed, 4096, "only the present file frees space");
    assert!(failures.is_empty(), "a missing path is not a failure");
    assert!(!present.exists());
}

#[test]
fn clean_emits_progress_for_every_item() {
    let dir = tempdir().unwrap();
    let mut items = Vec::new();
    for i in 0..2 {
        let p = dir.path().join(format!("p{i}.bin"));
        write_file(&p, &[0u8; 4096]);
        items.push(ScanItem::new(
            p,
            "p",
            4096,
            SafetyTier::Safe,
            Category::Caches,
        ));
    }

    let (progress, freed) = clean_collect(
        items,
        CleanOptions {
            permanent: true,
            dry_run: false,
            mode: DeleteMode::Permanent,
        },
    );

    assert_eq!(progress.len(), 2, "one progress tick per item");
    assert_eq!(
        progress.last(),
        Some(&(2, 2)),
        "final tick reports done == total"
    );
    assert_eq!(freed, 4096 * 2);
}

#[test]
fn clean_trash_category_deletes_permanently_even_in_trash_mode() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("in_trash.bin");
    write_file(&p, &[0u8; 4096]);
    let item = ScanItem::new(
        p.clone(),
        "in_trash",
        4096,
        SafetyTier::Safe,
        Category::Trash,
    );

    // Trash mode, not forced permanent — the Trash category still deletes for good.
    let (freed, failures) = clean_and_wait(
        vec![item],
        CleanOptions {
            permanent: false,
            dry_run: false,
            mode: DeleteMode::Trash,
        },
    );

    assert!(failures.is_empty());
    assert_eq!(freed, 4096);
    assert!(
        !p.exists(),
        "trash-category items are removed, not re-trashed"
    );
}

/// Moves a file to the real macOS Trash. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn clean_trash_roundtrip() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("to_trash.bin");
    write_file(&p, &[0u8; 4096]);
    let item = ScanItem::new(
        p.clone(),
        "to_trash",
        4096,
        SafetyTier::Safe,
        Category::Caches,
    );

    let (freed, failures) = clean_and_wait(
        vec![item],
        CleanOptions {
            permanent: false,
            dry_run: false,
            mode: DeleteMode::Trash,
        },
    );

    assert!(failures.is_empty(), "trash failed: {failures:?}");
    assert!(!p.exists(), "file should have moved to Trash");
    assert_eq!(freed, 4096);
}
