use mac_cleaner::clean::{CleanOptions, run_clean};
use mac_cleaner::config::Config;
use mac_cleaner::event::{Event, WorkerMsg, WorkerSender};
use mac_cleaner::model::{Category, ScanItem};
use mac_cleaner::scan::{self, ScanContext};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    let mut f = fs::File::create(path).expect("create file");
    f.write_all(bytes).expect("write file");
    f.flush().expect("flush file");
}

pub fn path_str(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

pub fn ctx_for(cfg: &Config, cat: Category) -> ScanContext {
    ScanContext {
        config: Arc::new(cfg.clone()),
        matchers: cfg.matchers().expect("build matchers"),
        tx: WorkerSender::null(),
        categories: vec![cat],
        limits: Arc::new(scan::ScanLimits::auto(1)),
    }
}

pub fn clean_and_wait(items: Vec<ScanItem>, opts: CleanOptions) -> (u64, Vec<String>) {
    let (worker, rx) = WorkerSender::channel();
    run_clean(items, opts, worker);
    loop {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Worker(WorkerMsg::CleanDone { freed, failures })) => {
                return (freed, failures);
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out waiting for CleanDone"),
        }
    }
}

pub fn clean_collect(items: Vec<ScanItem>, opts: CleanOptions) -> (Vec<(usize, usize)>, u64) {
    let (worker, rx) = WorkerSender::channel();
    run_clean(items, opts, worker);
    let mut progress = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Worker(WorkerMsg::CleanProgress { done, total, .. })) => {
                progress.push((done, total));
            }
            Ok(Event::Worker(WorkerMsg::CleanDone { freed, .. })) => {
                return (progress, freed);
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out waiting for CleanDone"),
        }
    }
}
