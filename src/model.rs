//! Core data model shared across scanning, cleaning, and the UI.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// How dangerous it is to remove an item. Drives default selection and color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyTier {
    /// Regenerable junk. Selected by default.
    Safe,
    /// Usually fine but may cost a rebuild / re-login / re-download.
    Moderate,
    /// User data or potentially destructive. Never selected by default.
    Risky,
}

impl SafetyTier {
    /// Multiplier used when ranking items by reclaim priority.
    pub fn regen_factor(self) -> f64 {
        match self {
            SafetyTier::Safe => 1.0,
            SafetyTier::Moderate => 0.7,
            SafetyTier::Risky => 0.3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SafetyTier::Safe => "safe",
            SafetyTier::Moderate => "moderate",
            SafetyTier::Risky => "risky",
        }
    }
}

/// The top-level cleaning categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Caches,
    Logs,
    DevArtifacts,
    Duplicates,
    ICloud,
    LargeFiles,
    Trash,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Caches,
        Category::Logs,
        Category::DevArtifacts,
        Category::Duplicates,
        Category::ICloud,
        Category::LargeFiles,
        Category::Trash,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::Caches => "Caches",
            Category::Logs => "Logs",
            Category::DevArtifacts => "Dev Artifacts",
            Category::Duplicates => "Duplicates",
            Category::ICloud => "iCloud Offload",
            Category::LargeFiles => "Large Files",
            Category::Trash => "Trash Bin",
        }
    }

    /// A short slug for CLI `--categories` parsing and JSON output.
    pub fn slug(self) -> &'static str {
        match self {
            Category::Caches => "caches",
            Category::Logs => "logs",
            Category::DevArtifacts => "dev",
            Category::Duplicates => "duplicates",
            Category::ICloud => "icloud",
            Category::LargeFiles => "large",
            Category::Trash => "trash",
        }
    }

    pub fn from_slug(s: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.slug() == s)
    }

    pub fn icon(self) -> &'static str {
        match self {
            Category::Caches => "\u{2699}",
            Category::Logs => "\u{1F4C4}",
            Category::DevArtifacts => "\u{1F6E0}",
            Category::Duplicates => "\u{29C9}",
            Category::ICloud => "\u{2601}",
            Category::LargeFiles => "\u{1F4E6}",
            Category::Trash => "\u{1F5D1}",
        }
    }
}

/// Main panel: overview chart or a category item list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainView {
    Dashboard,
    Category(Category),
}

impl MainView {
    pub fn category(self) -> Option<Category> {
        match self {
            MainView::Dashboard => None,
            MainView::Category(c) => Some(c),
        }
    }
}

/// What the cleaner should do with an item when the user confirms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemAction {
    /// Move the path to Trash (or permanently delete in `--permanent`).
    Delete,
    /// Truncate a still-open log file in place instead of deleting it.
    Truncate,
    /// `brctl evict` the path: free the local copy, keep it in iCloud.
    Evict,
    /// Empty the Trash bin (intentionally permanent).
    EmptyTrash,
    /// Run a Docker prune subcommand rather than touching files directly.
    DockerPrune(DockerPrune),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockerPrune {
    BuildCache,
    Images,
    Containers,
    Volumes,
}

impl DockerPrune {
    pub fn args(self) -> &'static [&'static str] {
        match self {
            DockerPrune::BuildCache => &["builder", "prune", "-f"],
            DockerPrune::Images => &["image", "prune", "-af"],
            DockerPrune::Containers => &["container", "prune", "-f"],
            DockerPrune::Volumes => &["volume", "prune", "-f"],
        }
    }
}

/// One reclaimable thing found by a scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanItem {
    pub path: PathBuf,
    pub label: String,
    pub real_bytes: u64,
    pub tier: SafetyTier,
    pub category: Category,
    pub last_access_days: u32,
    pub regen_note: String,
    #[serde(default)]
    pub selected: bool,
    /// Duplicate-set id; items sharing a value are byte-identical.
    #[serde(default)]
    pub group_id: Option<u64>,
    /// Within a duplicate set, the keeper (oldest) is locked from selection.
    #[serde(default)]
    pub is_keeper: bool,
    pub action: ItemAction,
}

impl ScanItem {
    pub fn new(
        path: PathBuf,
        label: impl Into<String>,
        real_bytes: u64,
        tier: SafetyTier,
        category: Category,
    ) -> Self {
        ScanItem {
            path,
            label: label.into(),
            real_bytes,
            tier,
            category,
            last_access_days: 0,
            regen_note: String::new(),
            selected: false,
            group_id: None,
            is_keeper: false,
            action: ItemAction::Delete,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.regen_note = note.into();
        self
    }

    pub fn with_action(mut self, action: ItemAction) -> Self {
        self.action = action;
        self
    }

    pub fn with_age(mut self, days: u32) -> Self {
        self.last_access_days = days;
        self
    }

    /// Ranking score: big + regenerable + stale floats to the top.
    pub fn priority(&self) -> f64 {
        self.real_bytes as f64 * self.tier.regen_factor() * staleness_factor(self.last_access_days)
    }

    /// A keeper in a duplicate set can never be selected for deletion.
    pub fn selectable(&self) -> bool {
        !self.is_keeper
    }
}

/// Older files are slightly better deletion candidates.
pub fn staleness_factor(days: u32) -> f64 {
    match days {
        0..=6 => 0.8,
        7..=29 => 1.0,
        30..=89 => 1.2,
        _ => 1.5,
    }
}

/// Progress/lifecycle of a single category scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanStatus {
    Pending,
    Scanning { found: usize, bytes: u64 },
    Done,
    Skipped(String),
}

/// Aggregated scan output across all categories.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanResults {
    pub items: HashMap<Category, Vec<ScanItem>>,
    pub status: HashMap<Category, ScanStatus>,
}

impl ScanResults {
    pub fn new() -> Self {
        let mut status = HashMap::new();
        for cat in Category::ALL {
            status.insert(cat, ScanStatus::Pending);
        }
        ScanResults {
            items: HashMap::new(),
            status,
        }
    }

    pub fn items_for(&self, cat: Category) -> &[ScanItem] {
        self.items.get(&cat).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn total_bytes(&self, cat: Category) -> u64 {
        self.items_for(cat).iter().map(|i| i.real_bytes).sum()
    }

    pub fn selected_items(&self) -> Vec<&ScanItem> {
        self.items
            .values()
            .flat_map(|v| v.iter())
            .filter(|i| i.selected && i.selectable())
            .collect()
    }

    pub fn selected_bytes(&self) -> u64 {
        self.selected_items().iter().map(|i| i.real_bytes).sum()
    }

    pub fn total_reclaimable(&self) -> u64 {
        Category::ALL.iter().map(|c| self.total_bytes(*c)).sum()
    }

    /// After a category scan completes, sort by priority and pre-select Safe items.
    pub fn ingest(&mut self, cat: Category, mut items: Vec<ScanItem>) {
        items.sort_by(|a, b| {
            b.priority()
                .partial_cmp(&a.priority())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for item in &mut items {
            if item.tier == SafetyTier::Safe && item.selectable() {
                item.selected = true;
            }
        }
        self.items.insert(cat, items);
        self.status.insert(cat, ScanStatus::Done);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn item(bytes: u64, tier: SafetyTier) -> ScanItem {
        ScanItem::new(PathBuf::from("/x"), "x", bytes, tier, Category::Caches)
    }

    #[test]
    fn category_slug_roundtrips() {
        for cat in Category::ALL {
            assert_eq!(Category::from_slug(cat.slug()), Some(cat));
        }
        assert_eq!(Category::from_slug("nope"), None);
    }

    #[test]
    fn staleness_increases_with_age() {
        assert!(staleness_factor(0) < staleness_factor(10));
        assert!(staleness_factor(10) < staleness_factor(60));
        assert!(staleness_factor(60) < staleness_factor(365));
    }

    #[test]
    fn safe_outranks_risky_at_equal_size() {
        let safe = item(1000, SafetyTier::Safe);
        let risky = item(1000, SafetyTier::Risky);
        assert!(safe.priority() > risky.priority());
    }

    #[test]
    fn keeper_is_not_selectable() {
        let mut it = item(1000, SafetyTier::Safe);
        assert!(it.selectable());
        it.is_keeper = true;
        assert!(!it.selectable());
    }

    #[test]
    fn ingest_sorts_by_priority_and_preselects_safe() {
        let mut r = ScanResults::new();
        let mut keeper = item(4000, SafetyTier::Safe);
        keeper.is_keeper = true;
        r.ingest(
            Category::Caches,
            vec![
                item(10, SafetyTier::Safe),
                item(5000, SafetyTier::Safe),
                item(6000, SafetyTier::Risky),
                keeper,
            ],
        );
        let items = r.items_for(Category::Caches);

        for pair in items.windows(2) {
            assert!(pair[0].priority() >= pair[1].priority());
        }
        assert_eq!(items[0].real_bytes, 5000);

        let selected: Vec<u64> = items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.real_bytes)
            .collect();
        assert!(selected.contains(&5000));
        assert!(selected.contains(&10));
        assert!(!selected.contains(&6000));
        assert!(!selected.contains(&4000));
    }

    #[test]
    fn selected_bytes_counts_only_selectable_selected() {
        let mut r = ScanResults::new();
        let mut a = item(100, SafetyTier::Safe);
        a.selected = true;
        let mut keeper = item(200, SafetyTier::Safe);
        keeper.selected = true;
        keeper.is_keeper = true;
        let b = item(300, SafetyTier::Safe);
        r.items.insert(Category::Caches, vec![a, keeper, b]);
        assert_eq!(r.selected_items().len(), 1);
        assert_eq!(r.selected_bytes(), 100);
    }

    #[test]
    fn total_reclaimable_sums_all_categories() {
        let mut r = ScanResults::new();
        r.items
            .insert(Category::Caches, vec![item(100, SafetyTier::Safe)]);
        r.items
            .insert(Category::Logs, vec![item(250, SafetyTier::Safe)]);
        assert_eq!(r.total_reclaimable(), 350);
    }
}
