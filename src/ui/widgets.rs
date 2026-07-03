//! Reusable small widgets.

use crate::fs_util::human_size;

pub fn spinner_frame(tick: u64) -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

pub fn format_selected(count: usize, bytes: u64) -> String {
    format!("{count} selected — {}", human_size(bytes))
}
