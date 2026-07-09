use mac_cleaner::config::Config;
use mac_cleaner::model::{Category, SafetyTier, ScanResults};
use tempfile::tempdir;

use crate::common::{ctx_for, path_str, write_file};

#[test]
fn given_generated_dirs_when_scan_dev_artifacts_then_detect_and_prune_subtrees() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("rust/target/debug/app"), &[0u8; 8192]);
    write_file(&root.join("rust/target/build/nested"), &[0u8; 8192]);
    write_file(&root.join("web/node_modules/pkg/index.js"), &[0u8; 8192]);
    write_file(&root.join("infra/.terraform/providers/aws"), &[0u8; 8192]);
    write_file(&root.join("flutter/.dart_tool/build"), &[0u8; 8192]);
    write_file(&root.join("Chrome/Default/target/secret"), &[0u8; 8192]);
    write_file(
        &root.join("Visual Studio Code.app/Contents/Resources/app/node_modules/pkg/index.js"),
        &[0u8; 8192],
    );

    let mut cfg = Config::default();
    cfg.dev_artifacts.roots = vec![path_str(root)];
    cfg.dev_artifacts.review_roots = vec![];

    let items = mac_cleaner::scan::dev_artifacts::scan(&ctx_for(&cfg, Category::DevArtifacts))
        .expect("scan dev artifacts");

    assert_eq!(items.len(), 4, "expected 4 artifacts, got {items:?}");
    assert!(items.iter().any(|i| i.path.ends_with("target")));
    assert!(items.iter().any(|i| i.path.ends_with("node_modules")));
    assert!(items.iter().any(|i| i.path.ends_with(".terraform")));
    assert!(items.iter().any(|i| i.path.ends_with(".dart_tool")));
    assert!(items.iter().all(|i| i.category == Category::DevArtifacts));
    assert!(items.iter().all(|i| i.tier == SafetyTier::Moderate));
}

#[test]
fn given_dev_artifacts_when_ingest_then_review_first_not_auto_selected() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    write_file(&root.join("app/target/debug/app"), &[0u8; 8192]);

    let mut cfg = Config::default();
    cfg.dev_artifacts.roots = vec![path_str(root)];
    cfg.dev_artifacts.review_roots = vec![];

    let items = mac_cleaner::scan::dev_artifacts::scan(&ctx_for(&cfg, Category::DevArtifacts))
        .expect("scan dev artifacts");
    let mut results = ScanResults::new();
    results.ingest(Category::DevArtifacts, items);

    assert_eq!(results.items_for(Category::DevArtifacts).len(), 1);
    assert!(
        results
            .items_for(Category::DevArtifacts)
            .iter()
            .all(|i| !i.selected)
    );
}
