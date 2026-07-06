//! Configuration defaults and TOML overrides.
//!
//! `Config` stores user-facing values. `Matchers` stores the compiled glob and
//! name sets used by scanners.

use crate::fs_util::expand_tilde;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeleteMode {
    #[default]
    Trash,
    Permanent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Roots walked recursively for cache-signature directories.
    pub roots: Vec<String>,
    /// Directory-name signatures that mark a whole subtree as one cache item.
    pub signatures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LogsConfig {
    pub age_days: u32,
    pub roots: Vec<String>,
    /// Directory-name signatures (e.g. `logs`, `log`).
    pub dir_signatures: Vec<String>,
    /// File-name globs (e.g. `*.log`, `npm-debug.log*`).
    pub file_signatures: Vec<String>,
    /// Prefer truncating still-open logs over deleting them.
    pub truncate_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DuplicatesConfig {
    pub roots: Vec<String>,
    /// Ignore files smaller than this in duplicate detection.
    pub min_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LargeConfig {
    pub roots: Vec<String>,
    pub min_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivilegeConfig {
    pub auto_elevate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub cache: CacheConfig,
    pub logs: LogsConfig,
    pub duplicates: DuplicatesConfig,
    pub large: LargeConfig,
    pub privilege: PrivilegeConfig,
    /// Directory names skipped during recursive walks.
    pub exclude_dir_names: Vec<String>,
    /// Path components treated as user data.
    pub protected_names: Vec<String>,
    pub delete_mode: DeleteMode,
}

impl Default for CacheConfig {
    fn default() -> Self {
        CacheConfig {
            roots: vec![
                "~/Library/Caches".into(),
                "~/Library/Application Support".into(),
                "~/Library/Containers".into(),
                "~/Library/Group Containers".into(),
                "~/.cache".into(),
                "~/.npm/_cacache".into(),
                "~/.cargo/registry/cache".into(),
                "~/.gradle/caches".into(),
                "~/.cocoapods".into(),
                "~/Library/Developer/Xcode/DerivedData".into(),
                "~/Library/Developer/Xcode/iOS DeviceSupport".into(),
            ],
            signatures: vec![
                "Cache".into(),
                "Caches".into(),
                ".cache".into(),
                "Code Cache".into(),
                "GPUCache".into(),
                "GPU Cache".into(),
                "CachedData".into(),
                "CachedExtensionVSIXs".into(),
                "ShaderCache".into(),
                "DawnCache".into(),
                "DawnGraphiteCache".into(),
                "DawnWebGPUCache".into(),
                "GraphiteDawnCache".into(),
                "Cache_Data".into(),
                "component_crx_cache".into(),
                "CacheStorage".into(),
                "blob_storage".into(),
                "Crashpad".into(),
                "*.ShipIt".into(),
                "ShipIt".into(),
            ],
        }
    }
}

impl Default for LogsConfig {
    fn default() -> Self {
        LogsConfig {
            age_days: 7,
            roots: vec!["~".into(), "~/Library/Logs".into()],
            dir_signatures: vec!["logs".into(), "log".into()],
            file_signatures: vec![
                "*.log".into(),
                "*.log.*".into(),
                "npm-debug.log*".into(),
                "*-debug.log".into(),
                "crash*.log".into(),
            ],
            truncate_active: true,
        }
    }
}

impl Default for DuplicatesConfig {
    fn default() -> Self {
        DuplicatesConfig {
            roots: vec![
                "~/Downloads".into(),
                "~/Documents".into(),
                "~/Desktop".into(),
            ],
            min_bytes: 1024 * 1024, // 1 MB
        }
    }
}

impl Default for LargeConfig {
    fn default() -> Self {
        LargeConfig {
            roots: vec![
                "~/Downloads".into(),
                "~/Documents".into(),
                "~/Desktop".into(),
                "~/Movies".into(),
            ],
            min_bytes: 100 * 1024 * 1024, // 100 MB
        }
    }
}

impl Default for PrivilegeConfig {
    fn default() -> Self {
        PrivilegeConfig { auto_elevate: true }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            cache: CacheConfig::default(),
            logs: LogsConfig::default(),
            duplicates: DuplicatesConfig::default(),
            large: LargeConfig::default(),
            privilege: PrivilegeConfig::default(),
            exclude_dir_names: vec![
                "node_modules".into(),
                "target".into(),
                ".git".into(),
                "vendor".into(),
                ".venv".into(),
                "venv".into(),
                "Pods".into(),
            ],
            protected_names: vec![
                "User".into(),
                "Default".into(),
                "Local Storage".into(),
                "IndexedDB".into(),
                "databases".into(),
                "Cookies".into(),
                "Login Data".into(),
                "Keychains".into(),
                ".ssh".into(),
                ".gnupg".into(),
                "Steam".into(),
                "minecraft".into(),
                "Photos Library.photoslibrary".into(),
                "Mail".into(),
            ],
            delete_mode: DeleteMode::Trash,
        }
    }
}

impl Config {
    /// Standard config path (`~/.config/mac-cleaner/config.toml`).
    pub fn default_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("dev", "maccleaner", "mac-cleaner")
            .map(|d| d.config_dir().join("config.toml"))
    }

    /// Load config from `path` (or the default path). Missing file => defaults.
    pub fn load(path: Option<&Path>) -> Result<Config> {
        let path = match path {
            Some(p) => p.to_path_buf(),
            None => match Config::default_path() {
                Some(p) => p,
                None => return Ok(Config::default()),
            },
        };
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))?;
        Ok(cfg)
    }

    /// Write the current config to the default path.
    pub fn save_default(&self) -> Result<PathBuf> {
        let path = Config::default_path().context("no config directory available")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        Ok(path)
    }

    /// Expanded cache roots that actually exist on disk.
    pub fn cache_roots(&self) -> Vec<PathBuf> {
        existing(&self.cache.roots)
    }

    pub fn log_roots(&self) -> Vec<PathBuf> {
        existing(&self.logs.roots)
    }

    pub fn duplicate_roots(&self) -> Vec<PathBuf> {
        existing(&self.duplicates.roots)
    }

    pub fn large_roots(&self) -> Vec<PathBuf> {
        existing(&self.large.roots)
    }

    /// Build the compiled matchers once for a scan run.
    pub fn matchers(&self) -> Result<Matchers> {
        Matchers::build(self)
    }
}

fn existing(list: &[String]) -> Vec<PathBuf> {
    list.iter()
        .map(|s| expand_tilde(s))
        .filter(|p| p.exists())
        .collect()
}

/// Precompiled pattern matchers derived from a [`Config`].
pub struct Matchers {
    cache_sig: GlobSet,
    log_dir_sig: GlobSet,
    log_file_sig: GlobSet,
    exclude_names: HashSet<String>,
    protected_names: HashSet<String>,
}

impl Matchers {
    fn build(cfg: &Config) -> Result<Matchers> {
        Ok(Matchers {
            cache_sig: build_globset(&cfg.cache.signatures)?,
            log_dir_sig: build_globset(&cfg.logs.dir_signatures)?,
            log_file_sig: build_globset(&cfg.logs.file_signatures)?,
            exclude_names: cfg.exclude_dir_names.iter().cloned().collect(),
            protected_names: cfg.protected_names.iter().cloned().collect(),
        })
    }

    /// True if a directory *name* marks a cache subtree.
    pub fn is_cache_signature(&self, name: &str) -> bool {
        self.cache_sig.is_match(name)
    }

    pub fn is_log_dir(&self, name: &str) -> bool {
        self.log_dir_sig.is_match(name)
    }

    pub fn is_log_file(&self, name: &str) -> bool {
        self.log_file_sig.is_match(name)
    }

    /// Directory name skipped during recursive scans.
    pub fn is_excluded_dir(&self, name: &str) -> bool {
        self.exclude_names.contains(name)
    }

    /// Protected path components plus Chromium `Profile N` directories.
    pub fn is_protected(&self, path: &Path) -> bool {
        for comp in path.components() {
            let name = comp.as_os_str().to_string_lossy();
            if self.protected_names.contains(name.as_ref()) {
                return true;
            }
            if name.starts_with("Profile ") {
                return true;
            }
        }
        false
    }
}

fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        builder.add(Glob::new(p).with_context(|| format!("bad glob {p}"))?);
    }
    Ok(builder.build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_signatures_match_expected() {
        let m = Config::default().matchers().unwrap();
        assert!(m.is_cache_signature("Cache"));
        assert!(m.is_cache_signature("Code Cache"));
        assert!(m.is_cache_signature("GPUCache"));
        assert!(m.is_cache_signature("com.microsoft.VSCode.ShipIt"));
        assert!(m.is_cache_signature("component_crx_cache"));
        assert!(!m.is_cache_signature("Documents"));
        assert!(!m.is_cache_signature("User"));
    }

    #[test]
    fn protected_paths_are_flagged() {
        let m = Config::default().matchers().unwrap();
        assert!(m.is_protected(Path::new(
            "/x/Application Support/Google/Chrome/Default/Cache"
        )));
        assert!(m.is_protected(Path::new("/x/Chrome/Profile 2/Cache")));
        assert!(m.is_protected(Path::new("/Users/me/.ssh/id_rsa")));
        assert!(!m.is_protected(Path::new("/Users/me/Library/Caches/com.spotify.client")));
    }

    #[test]
    fn log_signatures_match() {
        let m = Config::default().matchers().unwrap();
        assert!(m.is_log_dir("logs"));
        assert!(m.is_log_dir("log"));
        assert!(m.is_log_file("app.log"));
        assert!(m.is_log_file("npm-debug.log.1"));
        assert!(!m.is_log_file("app.txt"));
    }
}
