//! mac-cleaner integration tests: exercise the real scanners and the cleaning
//! engine against throwaway temp directories.

use mac_cleaner::clean::{run_clean, CleanOptions};
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

/// Run `run_clean` and block until its terminal `CleanDone` message.
fn clean_and_wait(items: Vec<ScanItem>, opts: CleanOptions) -> (u64, Vec<String>) {
    let (worker, rx) = WorkerSender::channel();
    run_clean(items, opts, worker);
    loop {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Worker(WorkerMsg::CleanDone { freed, failures })) => {
                return (freed, failures)
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
    // A real cache dir we DO want to find.
    write_file(&root.join("Cache/data.bin"), &[0u8; 2048]);
    // A cache dir hidden inside a dependency dir we must skip entirely.
    write_file(&root.join("node_modules/Cache/data.bin"), &[0u8; 2048]);
    // A cache dir under a protected profile dir we must skip entirely.
    write_file(&root.join("Default/Cache/data.bin"), &[0u8; 2048]);
    // A plain file that should never trigger on_dir.
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
    write_file(&root.join("b.bin"), &shared); // byte-identical to a.bin
    // Same size, different content: must be filtered by the hash gate.
    let mut other = vec![7u8; 8192];
    other[0] = 42;
    write_file(&root.join("c.bin"), &other);
    // Below the min-size threshold: ignored entirely.
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
    // Both members share a group id.
    assert_eq!(items[0].group_id, items[1].group_id);
    assert!(items[0].group_id.is_some());
    // The unique file must not appear.
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
    // Mirrors synthetic items like the "Docker not running" prompt.
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
    let mut item = ScanItem::new(p.clone(), "active.log", 10_000, SafetyTier::Safe, Category::Logs);
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

/// Moves a file to the real macOS Trash, so it is `#[ignore]`d by default to
/// avoid cluttering the developer's Trash. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn clean_trash_roundtrip() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("to_trash.bin");
    write_file(&p, &[0u8; 4096]);
    let item = ScanItem::new(p.clone(), "to_trash", 4096, SafetyTier::Safe, Category::Caches);

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
