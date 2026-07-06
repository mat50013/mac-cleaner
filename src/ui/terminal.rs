//! Terminal setup and teardown.

use crossterm::ExecutableCommand;
use crossterm::cursor::{Hide, Show};
use crossterm::terminal::{Clear, ClearType, SetSize};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use std::io::{self, Stdout, stdout};

/// Minimum columns and rows required by the TUI.
pub const MIN_WIDTH: u16 = 160;
pub const MIN_HEIGHT: u16 = 50;

/// Requested window size on startup.
pub const PREFERRED_WIDTH: u16 = 160;
pub const PREFERRED_HEIGHT: u16 = 50;

pub fn is_large_enough(width: u16, height: u16) -> bool {
    width >= MIN_WIDTH && height >= MIN_HEIGHT
}

/// Enter the TUI viewport.
pub fn prepare(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    let mut out = stdout();
    let _ = out.execute(SetSize(PREFERRED_WIDTH, PREFERRED_HEIGHT));
    out.execute(Clear(ClearType::All))?;
    out.execute(Hide)?;

    let (w, h) = crossterm::terminal::size().unwrap_or((PREFERRED_WIDTH, PREFERRED_HEIGHT));
    terminal.resize(Rect::new(0, 0, w, h))?;
    terminal.clear()?;
    Ok(())
}

/// Restore cursor visibility when leaving the TUI.
pub fn restore() {
    let _ = stdout().execute(Show);
}
