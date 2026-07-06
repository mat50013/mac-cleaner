//! Cache scanner.

use crate::fs_util::home_dir;
use crate::model::{Category, DockerPrune, ItemAction, SafetyTier, ScanItem};
use crate::scan::{
    ScanContext, item_from_dir, label_for, path_bytes, run_cmd, walk_parallel, which,
};
use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub fn scan(ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let items_mtx = Mutex::new(Vec::<ScanItem>::new());
    let seen = Mutex::new(HashSet::<PathBuf>::new());
    let total_bytes = AtomicU64::new(0);
    let count = AtomicUsize::new(0);

    for root in ctx.config.cache_roots() {
        let matchers = &ctx.matchers;
        let tx = ctx.tx.clone();

        walk_parallel(
            &root,
            matchers,
            |path, name| {
                if matchers.is_cache_signature(name) {
                    {
                        let mut guard = seen.lock().unwrap();
                        if !guard.insert(path.to_path_buf()) {
                            return true;
                        }
                    }
                    let bytes = path_bytes(path);
                    if bytes > 0 {
                        let label = label_for(path, "cache");
                        let tier = if name.contains("ShipIt") {
                            SafetyTier::Safe
                        } else if name.contains("DerivedData")
                            || name.contains("DeviceSupport")
                            || name.contains("cargo")
                        {
                            SafetyTier::Moderate
                        } else {
                            SafetyTier::Safe
                        };
                        let note = if name.contains("ShipIt") {
                            "updater leftover — safe to remove"
                        } else {
                            "regenerable cache"
                        };
                        items_mtx.lock().unwrap().push(item_from_dir(
                            path.to_path_buf(),
                            label,
                            bytes,
                            tier,
                            Category::Caches,
                            note,
                        ));
                        let running = total_bytes.fetch_add(bytes, Ordering::Relaxed) + bytes;
                        let found = count.fetch_add(1, Ordering::Relaxed) + 1;
                        tx.send(crate::event::WorkerMsg::ScanProgress {
                            category: Category::Caches,
                            found,
                            bytes: running,
                        });
                    }
                    return true;
                }
                false
            },
            |_path, _name| {},
        );
    }

    let mut items = items_mtx.into_inner().unwrap();

    let caches_root = home_dir().join("Library/Caches");
    if caches_root.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&caches_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let mut guard = seen.lock().unwrap();
                if !guard.insert(path.clone()) {
                    continue;
                }
                let bytes = path_bytes(&path);
                if bytes == 0 {
                    continue;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let tier = if name.starts_with("com.apple.") {
                    SafetyTier::Moderate
                } else {
                    SafetyTier::Safe
                };
                items.push(
                    ScanItem::new(path, name, bytes, tier, Category::Caches).with_note("app cache"),
                );
            }
        }
    }

    items.extend(scan_docker()?);
    items.sort_by(|a, b| b.real_bytes.cmp(&a.real_bytes));
    Ok(items)
}

fn scan_docker() -> Result<Vec<ScanItem>> {
    let mut items = Vec::new();
    if !which("docker") {
        return Ok(items);
    }

    let docker_raw =
        home_dir().join("Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw");
    if docker_raw.exists() {
        let bytes = path_bytes(&docker_raw);
        items.push(
            ScanItem::new(
                docker_raw,
                "Docker VM disk (sparse)",
                bytes,
                SafetyTier::Moderate,
                Category::Caches,
            )
            .with_note("logical size may be huge; shown size is real disk usage"),
        );
    }

    if run_cmd("docker", &["info"]).is_some() {
        if let Some(out) = run_cmd(
            "docker",
            &["system", "df", "--format", "{{.Type}}\t{{.Reclaimable}}"],
        ) {
            for line in out.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 2 {
                    continue;
                }
                let reclaimable = parts[1].trim();
                if reclaimable == "0B" || reclaimable.is_empty() {
                    continue;
                }
                let (label, action) = match parts[0] {
                    "Build Cache" => (
                        "Docker build cache",
                        ItemAction::DockerPrune(DockerPrune::BuildCache),
                    ),
                    "Images" => (
                        "Docker unused images",
                        ItemAction::DockerPrune(DockerPrune::Images),
                    ),
                    "Containers" => (
                        "Docker stopped containers",
                        ItemAction::DockerPrune(DockerPrune::Containers),
                    ),
                    "Local Volumes" => (
                        "Docker dangling volumes",
                        ItemAction::DockerPrune(DockerPrune::Volumes),
                    ),
                    _ => continue,
                };
                items.push(
                    ScanItem::new(
                        PathBuf::from("/docker-prune"),
                        format!("{label} ({reclaimable})"),
                        parse_docker_size(reclaimable),
                        SafetyTier::Moderate,
                        Category::Caches,
                    )
                    .with_note("docker prune — data may need re-pull")
                    .with_action(action),
                );
            }
        }
    } else {
        items.push(
            ScanItem::new(
                PathBuf::from("/docker-start"),
                "Docker not running — press Enter to start & rescan",
                0,
                SafetyTier::Moderate,
                Category::Caches,
            )
            .with_note("press Enter to launch Docker and rescan")
            .with_action(ItemAction::Delete),
        );
    }

    Ok(items)
}

fn parse_docker_size(s: &str) -> u64 {
    let s = s.trim();
    if let Some(num) = s.strip_suffix("GB") {
        (num.trim().parse::<f64>().unwrap_or(0.0) * 1_073_741_824.0) as u64
    } else if let Some(num) = s.strip_suffix("MB") {
        (num.trim().parse::<f64>().unwrap_or(0.0) * 1_048_576.0) as u64
    } else if let Some(num) = s.strip_suffix("KB") {
        (num.trim().parse::<f64>().unwrap_or(0.0) * 1024.0) as u64
    } else if let Some(num) = s.strip_suffix('B') {
        num.trim().parse().unwrap_or(0)
    } else {
        0
    }
}

/// Launch Docker Desktop and poll until the daemon responds.
pub fn start_docker_and_wait() -> bool {
    let _ = std::process::Command::new("open")
        .args(["-a", "Docker"])
        .status();
    for _ in 0..60 {
        if run_cmd("docker", &["info"]).is_some() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_docker_sizes() {
        assert_eq!(parse_docker_size("0B"), 0);
        assert_eq!(parse_docker_size("512B"), 512);
        assert_eq!(parse_docker_size("1KB"), 1024);
        assert_eq!(parse_docker_size("2MB"), 2 * 1_048_576);
        assert_eq!(parse_docker_size("1.5GB"), (1.5 * 1_073_741_824.0) as u64);
        assert_eq!(parse_docker_size(""), 0);
        assert_eq!(parse_docker_size("garbage"), 0);
        // Docker often pads the reclaimable column with spaces.
        assert_eq!(parse_docker_size("  3GB "), (3.0 * 1_073_741_824.0) as u64);
    }
}
