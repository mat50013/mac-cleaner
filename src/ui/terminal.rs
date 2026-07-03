//! Terminal setup: preferred size, minimum dimensions, cursor visibility.

use crossterm::cursor::{Hide, Show};
use crossterm::terminal::{Clear, ClearType, SetSize};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::{self, stdout, Stdout};

/// Minimum columns×rows before the UI refuses to render (shows a resize hint).
pub const MIN_WIDTH: u16 = 100;
pub const MIN_HEIGHT: u16 = 24;

/// Requested window size on startup (honoured by iTerm2/WezTerm; often ignored by Terminal.app).
pub const PREFERRED_WIDTH: u16 = 120;
pub const PREFERRED_HEIGHT: u16 = 40;

pub fn is_large_enough(width: u16, height: u16) -> bool {
    width >= MIN_WIDTH && height >= MIN_HEIGHT
}

/// Enter the dedicated TUI viewport: request size, clear, hide cursor.
pub fn prepare(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    let mut out = stdout();
    // Best-effort — Terminal.app may ignore this.
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
