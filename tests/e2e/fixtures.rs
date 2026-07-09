use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct E2eFixture {
    pub _tmp: TempDir,
    pub home: PathBuf,
    pub config_path: PathBuf,
}

impl E2eFixture {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).expect("create fake home");

        let cfg = tmp.path().join("config.toml");
        let config = format!(
            r#"
delete_mode = "permanent"

[cache]
roots = ["{home}/Library/Caches"]
signatures = ["Cache", "Caches", "DerivedData", "GPUCache", "Code Cache", ".cache", "ShipIt"]

[logs]
age_days = 7
roots = ["{home}/Library/Logs"]
dir_signatures = ["logs", "log"]
file_signatures = ["*.log", "*.log.*", "npm-debug.log*"]
truncate_active = true

[duplicates]
roots = ["{home}/Downloads"]
min_bytes = 1024

[dev_artifacts]
roots = ["{home}/Documents"]
review_roots = []
artifact_dir_names = ["target", "build", "dist", ".next", ".terraform", ".dart_tool"]
dependency_dir_names = ["node_modules", ".venv", "venv"]

[large]
roots = ["{home}/Downloads"]
min_bytes = 1048576
stale_archive_min_bytes = 1
stale_archive_days = 0

[privilege]
auto_elevate = false
"#,
            home = home.to_string_lossy()
        );
        std::fs::write(&cfg, config).expect("write config");

        Self {
            _tmp: tmp,
            home,
            config_path: cfg,
        }
    }

    pub fn write_file(&self, rel: &str, bytes: &[u8]) {
        let path = self.home.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, bytes).expect("write file");
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.home.join(rel)
    }

    pub fn normalize_paths(s: &str, home: &Path) -> String {
        s.replace(&home.to_string_lossy().to_string(), "$HOME")
    }
}
