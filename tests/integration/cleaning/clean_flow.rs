use mac_cleaner::clean::CleanOptions;
use mac_cleaner::config::DeleteMode;
use mac_cleaner::model::{Category, ItemAction, SafetyTier, ScanItem};
use std::path::PathBuf;
use tempfile::tempdir;

use crate::common::{clean_and_wait, clean_and_wait_with_timeout, clean_collect, write_file};

#[test]
fn given_dry_run_when_cleaning_then_reports_bytes_without_deleting() {
    let dir = tempdir().expect("tempdir");
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

    assert!(p.exists());
    assert_eq!(freed, 4096);
    assert!(failures.is_empty());
}

#[test]
fn given_permanent_mode_when_cleaning_then_removes_file() {
    let dir = tempdir().expect("tempdir");
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

    assert!(!p.exists());
    assert_eq!(freed, 4096);
    assert!(failures.is_empty());
}

#[test]
fn given_missing_path_when_cleaning_then_noop_without_error() {
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
    assert_eq!(freed, 0);
    assert!(failures.is_empty());
}

#[test]
fn given_truncate_action_when_cleaning_then_file_kept_and_zeroed() {
    let dir = tempdir().expect("tempdir");
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

    assert!(failures.is_empty());
    assert!(p.exists());
    assert_eq!(std::fs::metadata(&p).expect("metadata").len(), 0);
}

#[test]
fn given_multiple_files_when_cleaning_then_reports_total_freed() {
    let dir = tempdir().expect("tempdir");
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
    assert!(paths.iter().all(|p| !p.exists()));
}

#[test]
fn given_mixed_present_and_missing_when_cleaning_then_counts_only_existing() {
    let dir = tempdir().expect("tempdir");
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
    assert_eq!(freed, 4096);
    assert!(failures.is_empty());
    assert!(!present.exists());
}

#[test]
fn given_clean_run_when_collecting_progress_then_tick_per_item_and_final_done_total() {
    let dir = tempdir().expect("tempdir");
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
    assert_eq!(progress, vec![(1, 2), (2, 2)]);
    assert_eq!(freed, 4096 * 2);
}

#[test]
fn given_trash_category_when_cleaning_in_trash_mode_then_deletes_permanently() {
    let dir = tempdir().expect("tempdir");
    let p = dir.path().join("in_trash.bin");
    write_file(&p, &[0u8; 4096]);
    let item = ScanItem::new(
        p.clone(),
        "in_trash",
        4096,
        SafetyTier::Safe,
        Category::Trash,
    );

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
    assert!(!p.exists());
}

/// Moves a file to the real macOS Trash. Run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn given_trash_mode_when_cleaning_then_moves_file_to_user_trash() {
    let dir = tempdir().expect("tempdir");
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
    if failures.is_empty() {
        assert!(!p.exists());
        assert_eq!(freed, 4096);
        return;
    }

    // Headless CI/sandbox may not provide Finder AppleScript integration.
    let msg = failures.join("\n");
    assert!(
        msg.contains("Finder")
            || msg.contains("Connection invalid")
            || msg.contains("AppleScript"),
        "unexpected trash failure: {failures:?}"
    );
    assert!(p.exists(), "when Finder integration is unavailable, file should remain");
    assert_eq!(freed, 0);
}

#[test]
fn given_empty_trash_action_when_run_then_completes_without_failure() {
    let home = tempdir().expect("tempdir");
    let home_path = home.path().to_path_buf();
    let old_home = std::env::var_os("HOME");
    // SAFETY: test restores HOME before exit.
    unsafe { std::env::set_var("HOME", &home_path) };
    let run = || {
        let item = ScanItem::new(
            PathBuf::from("/tmp/empty-trash"),
            "empty-trash",
            1234,
            SafetyTier::Moderate,
            Category::Trash,
        )
        .with_action(ItemAction::EmptyTrash);
        let (freed, failures) = clean_and_wait(
            vec![item],
            CleanOptions {
                permanent: false,
                dry_run: false,
                mode: DeleteMode::Trash,
            },
        );
        assert_eq!(freed, 1234);
        assert!(failures.is_empty(), "empty trash failures: {failures:?}");
    };
    run();
    match old_home {
        Some(v) => unsafe { std::env::set_var("HOME", v) },
        None => unsafe { std::env::remove_var("HOME") },
    }
}

/// Docker prune can mutate runner/container state and runtime can vary heavily.
/// Keep it opt-in so default CI stays deterministic.
#[test]
#[ignore]
fn given_docker_prune_action_when_invoked_then_completes_or_reports_failure() {
    let item = ScanItem::new(
        PathBuf::from("/docker"),
        "docker-prune",
        500,
        SafetyTier::Moderate,
        Category::Caches,
    )
    .with_action(ItemAction::DockerPrune(
        mac_cleaner::model::DockerPrune::Images,
    ));
    let (freed, failures) = clean_and_wait_with_timeout(
        vec![item],
        CleanOptions {
            permanent: false,
            dry_run: false,
            mode: DeleteMode::Trash,
        },
        std::time::Duration::from_secs(90),
    );
    // On some hosts docker is unavailable (expected failure), on others it succeeds.
    assert!(freed == 0 || freed == 500);
    if freed == 0 {
        assert!(!failures.is_empty(), "expected a docker error");
    }
}

#[test]
fn given_evict_action_when_brctl_fails_then_reports_failure() {
    let item = ScanItem::new(
        PathBuf::from("/definitely/not/icloud/file.mov"),
        "evict",
        777,
        SafetyTier::Moderate,
        Category::ICloud,
    )
    .with_action(ItemAction::Evict);
    let (freed, failures) = clean_and_wait(
        vec![item],
        CleanOptions {
            permanent: false,
            dry_run: false,
            mode: DeleteMode::Trash,
        },
    );
    assert_eq!(freed, 0);
    assert!(!failures.is_empty(), "expected brctl failure");
}
