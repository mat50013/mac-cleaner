//! Application state machine: input, scanning, selection, cleaning.

use crate::clean::{CleanOptions, run_clean};
use crate::config::{Config, DeleteMode};
use crate::event::{DiskInfo, Event, EventHandler, WorkerMsg, WorkerSender};
use crate::model::{Category, SafetyTier, ScanResults, ScanStatus};
use crate::privilege::{PrivilegeInfo, open_fda_settings};
use crate::scan::caches::start_docker_and_wait;
use crate::scan::{ScanContext, run_all};
use crate::ui::detail;
use crate::ui::modal::Modal;
use crate::ui::terminal::{PREFERRED_HEIGHT, PREFERRED_WIDTH};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use std::collections::HashSet;
use std::io;
use std::sync::Arc;

pub struct App {
    pub config: Config,
    pub results: ScanResults,
    pub current_category: Category,
    pub selected_row: usize,
    pub scroll: usize,
    pub scanning: bool,
    pub cleaning: bool,
    pub tick: u64,
    pub disk: DiskInfo,
    pub privilege: PrivilegeInfo,
    pub modal: Modal,
    pub status_line: String,
    pub show_dashboard: bool,
    categories: Vec<Category>,
    finished: HashSet<Category>,
    worker: WorkerSender,
    dry_run: bool,
    pending_clean_permanent: bool,
    terminal_width: u16,
    terminal_height: u16,
    pub detail_visible_rows: usize,
}

impl App {
    pub fn new(
        config: Config,
        privilege: PrivilegeInfo,
        worker: WorkerSender,
        categories: Vec<Category>,
        dry_run: bool,
    ) -> App {
        App {
            config,
            results: ScanResults::new(),
            current_category: Category::Caches,
            selected_row: 0,
            scroll: 0,
            scanning: false,
            cleaning: false,
            tick: 0,
            disk: DiskInfo::default(),
            privilege,
            modal: if !privilege.full_disk_access {
                Modal::Fda
            } else {
                Modal::None
            },
            status_line: String::new(),
            show_dashboard: true,
            categories,
            finished: HashSet::new(),
            worker,
            dry_run,
            pending_clean_permanent: false,
            terminal_width: PREFERRED_WIDTH,
            terminal_height: PREFERRED_HEIGHT,
            detail_visible_rows: detail::visible_data_rows_for_terminal(
                PREFERRED_WIDTH,
                PREFERRED_HEIGHT,
            ),
        }
    }

    pub fn start_scan(&mut self) {
        self.scanning = true;
        self.finished.clear();
        self.results = ScanResults::new();
        self.show_dashboard = false;
        self.status_line = "Starting scan…".into();

        for cat in &self.categories {
            self.results.status.insert(*cat, ScanStatus::Pending);
        }

        let matchers = self.config.matchers().expect("matchers");
        let ctx = ScanContext {
            config: Arc::new(self.config.clone()),
            matchers,
            tx: self.worker.clone(),
            categories: self.categories.clone(),
        };
        run_all(ctx);
    }

    /// Drive the TUI event loop.
    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        handler: EventHandler,
    ) -> io::Result<()> {
        self.start_scan();

        loop {
            if let Ok(size) = terminal.size() {
                self.terminal_width = size.width;
                self.terminal_height = size.height;
            }
            terminal.draw(|f| crate::ui::draw(f, self))?;

            if let Some(ev) = handler.next() {
                match ev {
                    Event::Tick => {
                        self.tick = self.tick.wrapping_add(1);
                    }
                    Event::Resize => {
                        let size = terminal.size()?;
                        self.terminal_width = size.width;
                        self.terminal_height = size.height;
                        terminal.resize(Rect::new(0, 0, size.width, size.height))?;
                    }
                    Event::Worker(msg) => self.on_worker(msg),
                    Event::Input(input) => {
                        if self.handle_input(input) {
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn on_worker(&mut self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::Disk(d) => self.disk = d,
            WorkerMsg::ScanStarted(cat) => {
                self.results
                    .status
                    .insert(cat, ScanStatus::Scanning { found: 0, bytes: 0 });
                self.status_line = format!("Scanning {}…", cat.title());
            }
            WorkerMsg::ScanProgress {
                category,
                found,
                bytes,
            } => {
                self.results
                    .status
                    .insert(category, ScanStatus::Scanning { found, bytes });
                self.status_line = format!(
                    "Scanning {} — {found} items, {}",
                    category.title(),
                    crate::fs_util::human_size(bytes)
                );
            }
            WorkerMsg::ScanDone { category, items } => {
                self.results.ingest(category, items);
                self.finished.insert(category);
                if self.finished.len() >= self.categories.len() {
                    self.scanning = false;
                    self.show_dashboard = true;
                    self.status_line = "Scan complete".into();
                    self.worker.send(WorkerMsg::ScanComplete);
                }
            }
            WorkerMsg::ScanSkipped { category, reason } => {
                self.results
                    .status
                    .insert(category, ScanStatus::Skipped(reason.clone()));
                self.finished.insert(category);
                if self.finished.len() >= self.categories.len() {
                    self.scanning = false;
                    self.show_dashboard = true;
                }
            }
            WorkerMsg::ScanComplete => {}
            WorkerMsg::CleanProgress { done, total, freed } => {
                self.modal = Modal::CleanProgress { done, total, freed };
            }
            WorkerMsg::CleanDone { freed, failures } => {
                self.cleaning = false;
                self.modal = Modal::CleanDone { freed, failures };
                self.remove_cleaned();
            }
            WorkerMsg::DockerReady(ok) => {
                if ok {
                    self.start_scan();
                }
            }
        }
    }

    fn handle_input(&mut self, ev: crossterm::event::Event) -> bool {
        if let crossterm::event::Event::Key(key) = ev {
            if !matches!(self.modal, Modal::None) {
                return self.handle_modal_key(key);
            }
            return self.handle_main_key(key);
        }
        false
    }

    fn handle_modal_key(&mut self, key: KeyEvent) -> bool {
        match &self.modal {
            Modal::Help => {
                self.modal = Modal::None;
            }
            Modal::ConfirmClean { permanent, .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.pending_clean_permanent = *permanent;
                    self.modal = Modal::None;
                    self.do_clean();
                }
                KeyCode::Char('n') | KeyCode::Esc => self.modal = Modal::None,
                _ => {}
            },
            Modal::CleanDone { .. } => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q')) {
                    self.modal = Modal::None;
                }
            }
            Modal::Fda => match key.code {
                KeyCode::Char('o') | KeyCode::Char('O') => open_fda_settings(),
                KeyCode::Char('n') | KeyCode::Esc => self.modal = Modal::None,
                _ => {}
            },
            Modal::DockerStart => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.modal = Modal::None;
                    let tx = self.worker.clone();
                    std::thread::spawn(move || {
                        let ok = start_docker_and_wait();
                        tx.send(WorkerMsg::DockerReady(ok));
                    });
                }
                KeyCode::Char('n') | KeyCode::Esc => self.modal = Modal::None,
                _ => {}
            },
            Modal::CleanProgress { .. } => {}
            Modal::None => {}
        }
        key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL)
    }

    fn handle_main_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('?') => self.modal = Modal::Help,
            KeyCode::Char('r') => self.start_scan(),
            KeyCode::Char('d') => self.open_clean_confirm(false),
            KeyCode::Char('D') => self.open_clean_confirm(true),
            KeyCode::Char(' ') => self.toggle_current(),
            KeyCode::Char('a') => self.select_all_category(true),
            KeyCode::Char('A') => self.select_all_category(false),
            KeyCode::Char('s') => self.select_all_safe(),
            KeyCode::Char('n') => self.clear_selection(),
            KeyCode::Char('i') => self.invert_category(),
            KeyCode::Up | KeyCode::Char('k') => self.move_row(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_row(1),
            KeyCode::Tab => self.next_category(1),
            KeyCode::BackTab => self.next_category(-1),
            KeyCode::Enter => {
                if let Some(items) = self.results.items.get(&self.current_category) {
                    if let Some(item) = items.get(self.selected_row) {
                        if item.path == std::path::Path::new("/docker-start") {
                            self.modal = Modal::DockerStart;
                            return false;
                        }
                    }
                }
                self.flip_keeper();
            }
            _ => {}
        }
        false
    }

    fn toggle_current(&mut self) {
        if let Some(items) = self.results.items.get_mut(&self.current_category) {
            if let Some(item) = items.get_mut(self.selected_row) {
                if item.selectable() {
                    item.selected = !item.selected;
                }
            }
        }
    }

    fn select_all_category(&mut self, select: bool) {
        if let Some(items) = self.results.items.get_mut(&self.current_category) {
            for item in items.iter_mut() {
                if item.selectable() {
                    item.selected = select;
                }
            }
        }
    }

    fn select_all_safe(&mut self) {
        for items in self.results.items.values_mut() {
            for item in items.iter_mut() {
                if item.tier == SafetyTier::Safe && item.selectable() {
                    item.selected = true;
                } else {
                    item.selected = false;
                }
            }
        }
    }

    fn clear_selection(&mut self) {
        for items in self.results.items.values_mut() {
            for item in items.iter_mut() {
                item.selected = false;
            }
        }
    }

    fn invert_category(&mut self) {
        if let Some(items) = self.results.items.get_mut(&self.current_category) {
            for item in items.iter_mut() {
                if item.selectable() {
                    item.selected = !item.selected;
                }
            }
        }
    }

    fn move_row(&mut self, delta: i32) {
        let len = self.results.items_for(self.current_category).len();
        if len == 0 {
            return;
        }
        let row = self.selected_row as i32 + delta;
        self.selected_row = row.clamp(0, len as i32 - 1) as usize;

        let visible = self.detail_visible_rows.max(1);
        let max_scroll = len.saturating_sub(visible);

        if delta > 0 && self.selected_row >= self.scroll + visible {
            self.scroll = (self.scroll + 1).min(max_scroll);
        } else if delta < 0 && self.selected_row < self.scroll {
            self.scroll = self.scroll.saturating_sub(1);
        }
    }

    fn next_category(&mut self, delta: i32) {
        let all = &self.categories;
        let pos = all
            .iter()
            .position(|c| *c == self.current_category)
            .unwrap_or(0) as i32;
        let next = (pos + delta).rem_euclid(all.len() as i32) as usize;
        self.current_category = all[next];
        self.selected_row = 0;
        self.scroll = 0;
        self.show_dashboard = false;
    }

    fn flip_keeper(&mut self) {
        if self.current_category != Category::Duplicates {
            return;
        }
        let Some(items) = self.results.items.get_mut(&Category::Duplicates) else {
            return;
        };
        let row = self.selected_row;
        let gid = items.get(row).and_then(|i| i.group_id);
        let Some(gid) = gid else { return };
        let indices: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.group_id == Some(gid))
            .map(|(i, _)| i)
            .collect();
        for &i in &indices {
            items[i].is_keeper = i == row;
            items[i].selected = i != row;
            items[i].tier = if i == row {
                SafetyTier::Risky
            } else {
                SafetyTier::Safe
            };
        }
    }

    fn open_clean_confirm(&mut self, force_permanent: bool) {
        let selected: Vec<_> = self.results.selected_items();
        if selected.is_empty() {
            return;
        }
        let empty_trash = selected
            .iter()
            .all(|i| matches!(i.action, crate::model::ItemAction::EmptyTrash));
        let evict_only = selected
            .iter()
            .all(|i| matches!(i.action, crate::model::ItemAction::Evict));
        let in_trash = selected
            .iter()
            .all(|i| crate::fs_util::is_in_user_trash(&i.path) || i.category == Category::Trash);
        let permanent = force_permanent || empty_trash || in_trash;
        self.pending_clean_permanent = permanent;
        self.modal = Modal::ConfirmClean {
            count: selected.len(),
            bytes: self.results.selected_bytes(),
            permanent,
            empty_trash,
            evict_only,
        };
    }

    fn do_clean(&mut self) {
        let selected: Vec<_> = self
            .results
            .items
            .values()
            .flat_map(|v| v.iter())
            .filter(|i| i.selected && i.selectable())
            .cloned()
            .collect();
        if selected.is_empty() {
            return;
        }
        self.cleaning = true;
        let permanent =
            self.pending_clean_permanent || self.config.delete_mode == DeleteMode::Permanent;
        let opts = CleanOptions {
            permanent,
            dry_run: self.dry_run,
            mode: self.config.delete_mode,
        };
        run_clean(selected, opts, self.worker.clone());
        self.pending_clean_permanent = false;
    }

    fn remove_cleaned(&mut self) {
        for items in self.results.items.values_mut() {
            items.retain(|i| !(i.selected && i.selectable()));
            for item in items.iter_mut() {
                item.selected = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ScanItem;
    use std::path::PathBuf;

    fn app() -> App {
        let privilege = PrivilegeInfo {
            is_root: false,
            limited: true,
            full_disk_access: true,
        };
        App::new(
            Config::default(),
            privilege,
            WorkerSender::null(),
            Category::ALL.to_vec(),
            true,
        )
    }

    fn item(bytes: u64, tier: SafetyTier) -> ScanItem {
        ScanItem::new(
            PathBuf::from(format!("/x/{bytes}")),
            format!("i{bytes}"),
            bytes,
            tier,
            Category::Caches,
        )
    }

    fn selected_count(a: &App, cat: Category) -> usize {
        a.results
            .items_for(cat)
            .iter()
            .filter(|i| i.selected)
            .count()
    }

    #[test]
    fn select_all_and_deselect_respects_current_category() {
        let mut a = app();
        a.results.items.insert(
            Category::Caches,
            vec![
                item(1, SafetyTier::Safe),
                item(2, SafetyTier::Moderate),
                item(3, SafetyTier::Risky),
            ],
        );
        a.current_category = Category::Caches;

        a.select_all_category(true);
        assert_eq!(selected_count(&a, Category::Caches), 3);

        a.select_all_category(false);
        assert_eq!(selected_count(&a, Category::Caches), 0);
    }

    #[test]
    fn select_all_safe_spans_categories_but_only_safe() {
        let mut a = app();
        a.results.items.insert(
            Category::Caches,
            vec![item(1, SafetyTier::Safe), item(2, SafetyTier::Risky)],
        );
        a.results
            .items
            .insert(Category::Logs, vec![item(3, SafetyTier::Safe)]);

        a.select_all_safe();
        assert_eq!(selected_count(&a, Category::Caches), 1);
        assert_eq!(selected_count(&a, Category::Logs), 1);
    }

    #[test]
    fn toggle_and_move_row_clamps() {
        let mut a = app();
        a.results.items.insert(
            Category::Caches,
            vec![item(1, SafetyTier::Safe), item(2, SafetyTier::Safe)],
        );
        a.current_category = Category::Caches;
        a.selected_row = 0;

        a.toggle_current();
        assert!(a.results.items_for(Category::Caches)[0].selected);

        a.move_row(1);
        assert_eq!(a.selected_row, 1);
        a.move_row(50);
        assert_eq!(a.selected_row, 1);
        a.move_row(-50);
        assert_eq!(a.selected_row, 0);
    }

    #[test]
    fn move_row_scrolls_one_line_when_selection_leaves_view() {
        let mut a = app();
        let visible = a.detail_visible_rows;
        let count = visible + 5;
        a.results.items.insert(
            Category::LargeFiles,
            (0..count)
                .map(|i| item(i as u64 * 1000, SafetyTier::Moderate))
                .collect(),
        );
        a.current_category = Category::LargeFiles;

        for _ in 0..visible.saturating_sub(1) {
            a.move_row(1);
        }
        assert_eq!(a.selected_row, visible - 1);
        assert_eq!(a.scroll, 0);

        a.move_row(1);
        assert_eq!(a.selected_row, visible);
        assert_eq!(a.scroll, 1);

        a.move_row(-1);
        assert_eq!(a.selected_row, visible - 1);
        assert_eq!(a.scroll, 1);

        while a.selected_row > a.scroll {
            a.move_row(-1);
        }
        assert_eq!(a.selected_row, a.scroll);

        a.move_row(-1);
        assert_eq!(a.scroll, 0);
    }

    #[test]
    fn move_row_keeps_selection_on_screen() {
        let mut a = app();
        let visible = a.detail_visible_rows;
        let count = visible + 5;
        a.results.items.insert(
            Category::LargeFiles,
            (0..count)
                .map(|i| item(i as u64 * 1000, SafetyTier::Moderate))
                .collect(),
        );
        a.current_category = Category::LargeFiles;

        for _ in 0..count - 1 {
            a.move_row(1);
        }
        assert_eq!(a.selected_row, count - 1);
        assert!(a.selected_row < a.scroll + visible);
    }

    #[test]
    fn invert_flips_selectable_items() {
        let mut a = app();
        let mut sel = item(1, SafetyTier::Safe);
        sel.selected = true;
        a.results
            .items
            .insert(Category::Caches, vec![sel, item(2, SafetyTier::Safe)]);
        a.current_category = Category::Caches;

        a.invert_category();
        let items = a.results.items_for(Category::Caches);
        assert!(!items[0].selected);
        assert!(items[1].selected);
    }

    #[test]
    fn flip_keeper_moves_the_lock_within_a_group() {
        let mut a = app();
        let mut a0 = item(100, SafetyTier::Risky);
        a0.group_id = Some(1);
        a0.is_keeper = true;
        a0.selected = false;
        let mut a1 = item(100, SafetyTier::Safe);
        a1.group_id = Some(1);
        a1.is_keeper = false;
        a1.selected = true;
        a.results.items.insert(Category::Duplicates, vec![a0, a1]);
        a.current_category = Category::Duplicates;
        a.selected_row = 1;

        a.flip_keeper();
        let items = a.results.items_for(Category::Duplicates);
        assert!(items[1].is_keeper && !items[1].selected);
        assert_eq!(items[1].tier, SafetyTier::Risky);
        assert!(!items[0].is_keeper && items[0].selected);
        assert_eq!(items[0].tier, SafetyTier::Safe);
    }

    #[test]
    fn enter_on_docker_start_opens_modal() {
        let mut a = app();
        let mut docker = item(0, SafetyTier::Moderate);
        docker.path = PathBuf::from("/docker-start");
        a.results.items.insert(Category::Caches, vec![docker]);
        a.current_category = Category::Caches;
        a.selected_row = 0;

        let quit = a.handle_main_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!quit);
        assert!(matches!(a.modal, Modal::DockerStart));
    }

    #[test]
    fn remove_cleaned_drops_selected_keeps_locked() {
        let mut a = app();
        let mut sel = item(1, SafetyTier::Safe);
        sel.selected = true;
        let unsel = item(2, SafetyTier::Safe);
        let mut keeper = item(3, SafetyTier::Safe);
        keeper.selected = true;
        keeper.is_keeper = true;
        a.results
            .items
            .insert(Category::Caches, vec![sel, unsel, keeper]);

        a.remove_cleaned();
        let remaining: Vec<u64> = a
            .results
            .items_for(Category::Caches)
            .iter()
            .map(|i| i.real_bytes)
            .collect();
        assert!(!remaining.contains(&1));
        assert!(remaining.contains(&2));
        assert!(remaining.contains(&3));
    }
}
