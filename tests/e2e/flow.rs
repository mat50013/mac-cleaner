use serde_json::Value;
use std::process::Command;

use super::fixtures::E2eFixture;

fn run_bin(args: &[&str], home: &std::path::Path) -> std::process::Output {
    let exe = env!("CARGO_BIN_EXE_mac-cleaner");
    Command::new(exe)
        .env("HOME", home)
        .args(args)
        .output()
        .expect("run binary")
}

#[test]
fn scan_json_snapshot() {
    let fx = E2eFixture::new();
    fx.write_file("Library/Caches/App/Cache/data.bin", &vec![0u8; 1024 * 1024]);
    fx.write_file("Downloads/a.bin", &vec![7u8; 8192]);
    fx.write_file("Downloads/b.bin", &vec![7u8; 8192]);
    fx.write_file("Library/Logs/app.log", &vec![0u8; 4096]);

    let out = run_bin(
        &[
            "--no-elevate",
            "--config",
            &fx.config_path.to_string_lossy(),
            "scan",
            "--json",
        ],
        &fx.home,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("valid json");
    assert!(json.get("items").is_some());
    assert!(json.get("status").is_some());
    assert!(
        json["items"]["Caches"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "expected Caches results in JSON: {stdout}"
    );
}

#[test]
fn safe_clean_flow() {
    let fx = E2eFixture::new();
    let cache_rel = "Library/Caches/App/Cache/data.bin";
    fx.write_file(cache_rel, &vec![0u8; 2 * 1024 * 1024]);
    let cache_abs = fx.path(cache_rel);

    let out = run_bin(
        &[
            "--no-elevate",
            "--config",
            &fx.config_path.to_string_lossy(),
            "clean",
            "--categories",
            "caches",
            "--yes",
        ],
        &fx.home,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("Freed"), "stdout: {stdout}");
    assert!(
        !cache_abs.exists(),
        "cache file should be removed by clean flow"
    );
}

#[test]
fn duplicates_keep_one() {
    let fx = E2eFixture::new();
    fx.write_file("Downloads/a.bin", &vec![3u8; 4096]);
    fx.write_file("Downloads/b.bin", &vec![3u8; 4096]);
    fx.write_file("Downloads/c.bin", &vec![3u8; 4096]);

    let out = run_bin(
        &[
            "--no-elevate",
            "--config",
            &fx.config_path.to_string_lossy(),
            "clean",
            "--categories",
            "duplicates",
            "--yes",
            "--permanent",
        ],
        &fx.home,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let remaining = ["a.bin", "b.bin", "c.bin"]
        .iter()
        .filter(|n| fx.path(&format!("Downloads/{n}")).exists())
        .count();
    assert_eq!(remaining, 1, "exactly one duplicate keeper should remain");
}

#[test]
fn dry_run_no_side_effects() {
    let fx = E2eFixture::new();
    let rel = "Library/Caches/App/Cache/data.bin";
    fx.write_file(rel, &vec![0u8; 1024 * 1024]);
    let abs = fx.path(rel);

    let out = run_bin(
        &[
            "--no-elevate",
            "--dry-run",
            "--config",
            &fx.config_path.to_string_lossy(),
            "clean",
            "--categories",
            "caches",
            "--yes",
        ],
        &fx.home,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("Freed"), "stdout: {stdout}");
    assert!(abs.exists(), "dry-run must not modify filesystem");
}

#[test]
fn invalid_category_input_has_stable_behavior() {
    let fx = E2eFixture::new();
    let out = run_bin(
        &[
            "--no-elevate",
            "--config",
            &fx.config_path.to_string_lossy(),
            "scan",
            "--categories",
            "nope",
        ],
        &fx.home,
    );
    assert!(out.status.success(), "unexpected hard failure");
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let normalized = E2eFixture::normalize_paths(&stdout, &fx.home);
    assert!(
        normalized.contains("Total reclaimable: 0 B") || normalized.contains("##"),
        "stdout should remain parseable: {normalized}"
    );
}
