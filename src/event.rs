//! Event plumbing. A single [`crossbeam_channel`] carries three producers:
//! terminal input, a periodic tick, and background worker messages. The UI
//! thread only ever blocks on one receiver.

use crate::model::{Category, ScanItem};
use crossbeam_channel::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Messages emitted by background scan/clean workers back to the UI.
#[derive(Debug, Clone)]
pub enum WorkerMsg {
    ScanStarted(Category),
    ScanProgress {
        category: Category,
        found: usize,
        bytes: u64,
    },
    ScanDone {
        category: Category,
        items: Vec<ScanItem>,
    },
    ScanSkipped {
        category: Category,
        reason: String,
    },
    /// All categories finished.
    ScanComplete,
    CleanProgress {
        done: usize,
        total: usize,
        freed: u64,
    },
    CleanDone {
        freed: u64,
        failures: Vec<String>,
    },
    /// Docker daemon came up (or failed to) after a start request.
    DockerReady(bool),
    Disk(DiskInfo),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DiskInfo {
    pub total: u64,
    pub free: u64,
}

impl DiskInfo {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }

    pub fn used_ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.used() as f64 / self.total as f64
        }
    }
}

/// Everything the main loop reacts to.
#[derive(Debug, Clone)]
pub enum Event {
    Input(crossterm::event::Event),
    Tick,
    Resize,
    Worker(WorkerMsg),
}

/// Owns the receiving end plus a cloneable sender handed to workers.
pub struct EventHandler {
    rx: Receiver<Event>,
    tx: Sender<Event>,
}

impl EventHandler {
    /// Spawn the input+tick thread. `tick` is the redraw cadence (~30fps).
    pub fn new(tick: Duration) -> EventHandler {
        let (tx, rx) = crossbeam_channel::unbounded();
        let input_tx = tx.clone();
        thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(Duration::ZERO);
                // Poll for input up to the remaining tick budget.
                if crossterm::event::poll(timeout).unwrap_or(false) {
                    match crossterm::event::read() {
                        Ok(ev) => {
                            let event = if matches!(ev, crossterm::event::Event::Resize(_, _)) {
                                Event::Resize
                            } else {
                                Event::Input(ev)
                            };
                            if input_tx.send(event).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                if last_tick.elapsed() >= tick {
                    if input_tx.send(Event::Tick).is_err() {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });
        EventHandler { rx, tx }
    }

    /// A sender workers use to push [`WorkerMsg`]s.
    pub fn sender(&self) -> WorkerSender {
        WorkerSender {
            tx: self.tx.clone(),
        }
    }

    /// Block until the next event arrives.
    pub fn next(&self) -> Option<Event> {
        self.rx.recv().ok()
    }
}

/// Thin wrapper so worker code sends [`WorkerMsg`] without knowing about the
/// [`Event`] envelope.
#[derive(Clone)]
pub struct WorkerSender {
    tx: Sender<Event>,
}

impl WorkerSender {
    pub fn from_sender(tx: Sender<Event>) -> WorkerSender {
        WorkerSender { tx }
    }

    /// A sender whose messages are discarded. Useful for headless scans and
    /// tests that only care about the returned results, not progress events.
    pub fn null() -> WorkerSender {
        let (tx, _rx) = crossbeam_channel::unbounded();
        WorkerSender { tx }
    }

    /// A sender paired with a live receiver, so callers (e.g. tests) can observe
    /// exactly the [`WorkerMsg`]s that were emitted.
    pub fn channel() -> (WorkerSender, Receiver<Event>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        (WorkerSender { tx }, rx)
    }

    pub fn send(&self, msg: WorkerMsg) {
        let _ = self.tx.send(Event::Worker(msg));
    }
}
