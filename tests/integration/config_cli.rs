use clap::Parser;
use mac_cleaner::cli::{Cli, Commands};
use mac_cleaner::config::{Config, DeleteMode};
use mac_cleaner::fs_util::{dir_real_size, expand_tilde, human_size};
use mac_cleaner::model::Category;
use std::collections::HashSet;
use std::path::Path;
use tempfile::tempdir;

use crate::common::write_file;

#[test]
fn given_sizes_when_human_size_then_formats_consistently() {
    assert_eq!(human_size(1024 * 1024), "1.0 MB");
    assert_eq!(human_size(0), "0 B");
}

#[test]
fn given_tilde_paths_when_expand_then_resolves_home_dir() {
    let home = std::env::var("HOME").expect("HOME set");
    assert_eq!(expand_tilde("~").to_string_lossy(), home);
    assert_eq!(
        expand_tilde("~/Downloads").to_string_lossy(),
        format!("{home}/Downloads")
    );
}

#[test]
fn given_tree_when_dir_real_size_then_counts_children_once() {
    let dir = tempdir().expect("tempdir");
    write_file(&dir.path().join("a/one.bin"), &[1u8; 8192]);
    write_file(&dir.path().join("a/b/two.bin"), &[2u8; 8192]);
    let mut seen = HashSet::new();
    let total = dir_real_size(dir.path(), &mut seen);
    assert!(total >= 16384, "expected >= 16384, got {total}");
}

#[test]
fn given_config_when_matchers_then_signatures_and_guards_load() {
    let m = Config::default().matchers().expect("matchers");
    assert!(m.is_cache_signature("GPUCache"));
    assert!(m.is_log_dir("logs"));
    assert!(!m.is_protected(Path::new("/Users/x/Library/Caches/foo")));
    assert!(m.is_protected(Path::new("/Users/x/Chrome/Default")));
    assert!(m.is_excluded_dir("node_modules"));
}

#[test]
fn given_minimal_toml_when_load_then_defaults_still_apply() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("config.toml");
    write_file(
        &path,
        b"delete_mode = \"permanent\"\n\n[large]\nmin_bytes = 12345\n",
    );

    let cfg = Config::load(Some(&path)).expect("load config");
    assert_eq!(cfg.delete_mode, DeleteMode::Permanent);
    assert_eq!(cfg.large.min_bytes, 12345);
    assert_eq!(cfg.logs.age_days, 7);
    assert_eq!(cfg.duplicates.min_bytes, 1024 * 1024);
    assert!(
        cfg.dev_artifacts
            .artifact_dir_names
            .iter()
            .any(|x| x == "target")
    );
}

#[test]
fn given_dev_artifact_toml_when_load_then_overrides_parse() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("config.toml");
    write_file(
        &path,
        b"[dev_artifacts]\nroots = [\"/tmp/projects\"]\nreview_roots = []\nartifact_dir_names = [\"out\"]\ndependency_dir_names = [\"deps\"]\n",
    );

    let cfg = Config::load(Some(&path)).expect("load config");
    assert_eq!(cfg.dev_artifacts.roots, vec!["/tmp/projects"]);
    assert!(cfg.dev_artifacts.review_roots.is_empty());
    assert_eq!(cfg.dev_artifacts.artifact_dir_names, vec!["out"]);
    assert_eq!(cfg.dev_artifacts.dependency_dir_names, vec!["deps"]);
    assert_eq!(cfg.large.min_bytes, 100 * 1024 * 1024);
}

#[test]
fn given_missing_config_when_load_then_returns_defaults() {
    let cfg = Config::load(Some(Path::new("/no/such/mac-cleaner-config.toml"))).expect("load");
    assert_eq!(cfg.delete_mode, DeleteMode::Trash);
    assert_eq!(cfg.large.min_bytes, 100 * 1024 * 1024);
    assert_eq!(cfg.large.stale_archive_days, 30);
    assert!(cfg.dev_artifacts.roots.iter().any(|r| r == "~/Documents"));
}

#[test]
fn given_category_csv_when_parse_then_trims_and_drops_invalid() {
    let cats = Cli::parse_categories("caches, logs , dev, nope, duplicates");
    assert_eq!(
        cats,
        vec![
            Category::Caches,
            Category::Logs,
            Category::DevArtifacts,
            Category::Duplicates
        ]
    );
    assert!(Cli::parse_categories("").is_empty());
    assert!(Cli::parse_categories("bogus,,").is_empty());
}

#[test]
fn given_scan_cli_when_parse_then_subcommand_and_flags_are_kept() {
    let cli = Cli::try_parse_from(["mac-cleaner", "--dry-run", "scan", "--json"]).expect("parse");
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
fn given_no_subcommand_when_parse_then_defaults_to_tui() {
    let cli = Cli::try_parse_from(["mac-cleaner"]).expect("parse");
    assert!(cli.command.is_none());
    assert!(!cli.dry_run);
    assert!(!cli.no_elevate);
}
