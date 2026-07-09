use crossbeam_channel::RecvTimeoutError;
use mac_cleaner::config::Config;
use mac_cleaner::event::{Event, WorkerMsg, WorkerSender};
use mac_cleaner::model::Category;
use mac_cleaner::scan::{self, ScanContext};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;

use crate::common::{path_str, write_file};

#[test]
fn given_walk_parallel_when_excluded_or_protected_then_pruned() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("Cache/data.bin"), &[0u8; 2048]);
    write_file(&root.join("node_modules/Cache/data.bin"), &[0u8; 2048]);
    write_file(&root.join("Default/Cache/data.bin"), &[0u8; 2048]);
    write_file(&root.join("normal/file.txt"), &[0u8; 16]);

    let m = Config::default().matchers().expect("matchers");
    let found = Mutex::new(Vec::<PathBuf>::new());
    scan::walk_parallel(
        root,
        &m,
        1,
        |path, name| {
            if m.is_cache_signature(name) {
                found.lock().expect("lock").push(path.to_path_buf());
                true
            } else {
                false
            }
        },
        |_p, _n| {},
    );

    let found = found.into_inner().expect("into_inner");
    assert_eq!(found.len(), 1, "found: {found:?}");
    assert!(found[0].ends_with("Cache"));
}

#[test]
fn given_bounded_run_all_when_scanning_then_emits_terminal_events() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(
        &root.join("Library/Caches/App/Cache/data.bin"),
        &[0u8; 8192],
    );
    write_file(&root.join("logs/service.log"), &[0u8; 8192]);

    let mut cfg = Config::default();
    cfg.cache.roots = vec![path_str(root)];
    cfg.logs.roots = vec![path_str(root)];
    cfg.large.roots = vec![path_str(root)];
    cfg.duplicates.roots = vec![path_str(root)];
    cfg.dev_artifacts.roots = vec![path_str(root)];
    cfg.dev_artifacts.review_roots = vec![];

    let cats = vec![Category::Caches, Category::Logs];
    let (tx, rx) = crossbeam_channel::unbounded();
    let ctx = ScanContext {
        config: Arc::new(cfg.clone()),
        matchers: cfg.matchers().expect("matchers"),
        tx: WorkerSender::from_sender(tx),
        categories: cats.clone(),
        limits: Arc::new(scan::ScanLimits::auto(cats.len())),
    };
    scan::run_all(ctx);

    let mut done = HashSet::new();
    for _ in 0..200 {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Event::Worker(WorkerMsg::ScanDone { category, .. })) => {
                done.insert(category);
            }
            Ok(Event::Worker(WorkerMsg::ScanSkipped { category, .. })) => {
                done.insert(category);
            }
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if done.len() == cats.len() {
            break;
        }
    }

    assert_eq!(done.len(), cats.len(), "missing category completion events");
}
