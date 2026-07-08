//! Duplicate-file scanner.

use crate::fs_util::{birthtime, inode_id, real_size};
use crate::model::{Category, SafetyTier, ScanItem};
use crate::scan::{ScanContext, walk_parallel};
use anyhow::Result;
use blake3::Hasher;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const PARTIAL: usize = 4096;

pub fn scan(ctx: &ScanContext) -> Result<Vec<ScanItem>> {
    let files = collect_files(ctx);
    let groups = group_duplicates(&files, ctx.limits.hash_threads);

    let mut items = Vec::new();
    let mut group_id = 1u64;

    for mut indices in groups {
        if indices.len() < 2 {
            continue;
        }
        indices.sort_by_key(|&i| files[i].birth);
        for (pos, &i) in indices.iter().enumerate() {
            let f = &files[i];
            let is_keeper = pos == 0;
            let mut item = ScanItem::new(
                f.path.clone(),
                f.path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                f.bytes,
                if is_keeper {
                    SafetyTier::Risky
                } else {
                    SafetyTier::Safe
                },
                Category::Duplicates,
            )
            .with_note(if is_keeper {
                "keeper (oldest) — will not be deleted"
            } else {
                "duplicate of older file"
            });
            item.group_id = Some(group_id);
            item.is_keeper = is_keeper;
            item.selected = !is_keeper;
            items.push(item);
        }
        group_id += 1;
    }

    items.sort_by(|a, b| b.real_bytes.cmp(&a.real_bytes));
    Ok(items)
}

struct FileEntry {
    path: PathBuf,
    bytes: u64,
    birth: std::time::SystemTime,
}

/// Collect candidate files without hashing.
fn collect_files(ctx: &ScanContext) -> Vec<FileEntry> {
    let min = ctx.config.duplicates.min_bytes;
    let files_mtx = Mutex::new(Vec::<FileEntry>::new());
    let inodes_mtx = Mutex::new(HashSet::<(u64, u64)>::new());

    for root in ctx.config.duplicate_roots() {
        let matchers = &ctx.matchers;
        walk_parallel(
            &root,
            matchers,
            ctx.limits.walk_threads,
            |_p, _n| false,
            |path, _name| {
                let Ok(md) = std::fs::symlink_metadata(path) else {
                    return;
                };
                if !md.is_file() {
                    return;
                }
                let bytes = real_size(&md);
                if bytes < min {
                    return;
                }
                // Hard links share storage, so only one path per inode is kept.
                if md.nlink() > 1 && !inodes_mtx.lock().unwrap().insert(inode_id(&md)) {
                    return;
                }
                files_mtx.lock().unwrap().push(FileEntry {
                    path: path.to_path_buf(),
                    bytes,
                    birth: birthtime(&md).unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                });
            },
        );
    }

    files_mtx.into_inner().unwrap()
}

/// Return groups of byte-identical files by index.
fn group_duplicates(files: &[FileEntry], hash_threads: usize) -> Vec<Vec<usize>> {
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        by_size.entry(f.bytes).or_default().push(i);
    }
    let size_candidates: Vec<usize> = by_size
        .into_values()
        .filter(|g| g.len() > 1)
        .flatten()
        .collect();

    let pool = ThreadPoolBuilder::new()
        .num_threads(hash_threads.max(1))
        .thread_name(|i| format!("mac-cleaner-hash-{i}"))
        .build();

    match pool {
        Ok(pool) => pool.install(|| group_duplicates_by_hash(files, size_candidates)),
        Err(_) => group_duplicates_by_hash(files, size_candidates),
    }
}

fn group_duplicates_by_hash(files: &[FileEntry], size_candidates: Vec<usize>) -> Vec<Vec<usize>> {
    let partials: Vec<(usize, [u8; 32])> = size_candidates
        .par_iter()
        .filter_map(|&i| partial_hash(&files[i].path, files[i].bytes).map(|h| (i, h)))
        .collect();
    let mut partial_groups: HashMap<([u8; 32], u64), Vec<usize>> = HashMap::new();
    for (i, h) in partials {
        partial_groups
            .entry((h, files[i].bytes))
            .or_default()
            .push(i);
    }
    let full_candidates: Vec<usize> = partial_groups
        .into_values()
        .filter(|g| g.len() > 1)
        .flatten()
        .collect();

    let hashes: Vec<(usize, [u8; 32])> = full_candidates
        .par_iter()
        .filter_map(|&i| full_hash(&files[i].path).map(|h| (i, h)))
        .collect();
    let mut groups: HashMap<[u8; 32], Vec<usize>> = HashMap::new();
    for (i, h) in hashes {
        groups.entry(h).or_default().push(i);
    }

    groups.into_values().filter(|g| g.len() > 1).collect()
}

/// Hash the first and last [`PARTIAL`] bytes as a pre-filter.
fn partial_hash(path: &Path, size: u64) -> Option<[u8; 32]> {
    let mut f = File::open(path).ok()?;
    let mut h = Hasher::new();
    let mut buf = vec![0u8; PARTIAL];
    let n = f.read(&mut buf).ok()?;
    h.update(&buf[..n]);
    if size > PARTIAL as u64 * 2 {
        if f.seek(SeekFrom::End(-(PARTIAL as i64))).is_ok() {
            let mut tail = vec![0u8; PARTIAL];
            let n = f.read(&mut tail).ok()?;
            h.update(&tail[..n]);
        }
    }
    Some(*h.finalize().as_bytes())
}

/// Hash the full file contents in fixed-size chunks.
fn full_hash(path: &Path) -> Option<[u8; 32]> {
    const CHUNK: usize = 128 * 1024;
    let mut file = File::open(path).ok()?;
    let mut hasher = Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(*hasher.finalize().as_bytes())
}
