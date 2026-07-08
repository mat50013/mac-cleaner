//! Footer: status line plus key legend.

use crate::fs_util::human_size;
use crate::model::ScanResults;
use crate::ui::theme;
use crate::ui::widgets::{KeyHint, key_hint_line, key_hint_width, spinner_frame};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Outer footer height (including block borders) for layout.
pub fn outer_height(terminal_width: u16) -> u16 {
    let inner_w = inner_width(terminal_width);
    let content = 1 + key_legend(inner_w).len() as u16;
    content + 2
}

pub fn draw(
    f: &mut Frame,
    area: Rect,
    results: &ScanResults,
    scanning: bool,
    tick: u64,
    status: &str,
) {
    let block = theme::block("");
    let inner = block.inner(area);

    let mut lines = vec![status_line(results, scanning, tick, status)];
    lines.extend(key_legend(inner.width));

    f.render_widget(block, area);
    f.render_widget(Paragraph::new(lines), inner);
}

fn status_line(results: &ScanResults, scanning: bool, tick: u64, status: &str) -> Line<'static> {
    if scanning {
        return Line::from(vec![
            Span::styled(
                format!("{} Scanning… ", spinner_frame(tick)),
                Style::default().fg(theme::accent()),
            ),
            Span::styled(status.to_string(), theme::dim()),
        ]);
    }

    let count = results.selected_items().len();
    if count > 0 {
        Line::from(vec![
            Span::styled(
                format!(
                    "{count} selected · {}",
                    human_size(results.selected_bytes())
                ),
                Style::default().fg(theme::safe()).bold(),
            ),
            Span::styled("  —  press ", theme::dim()),
            Span::styled("d", theme::key_style()),
            Span::styled(" to clean", theme::dim()),
        ])
    } else {
        Line::from(Span::styled(
            "Nothing selected — Space toggles an item, s selects everything Safe",
            theme::dim(),
        ))
    }
}

fn inner_width(terminal_width: u16) -> u16 {
    terminal_width.saturating_sub(2)
}

const FULL: &[KeyHint] = &[
    ("Tab", "switch view"),
    ("Space", "select"),
    ("a", "all"),
    ("s", "all safe"),
    ("i", "invert"),
    ("n", "clear"),
    ("d", "clean"),
    ("D", "delete forever"),
    ("r", "rescan"),
    ("?", "help"),
    ("q", "quit"),
];

const MEDIUM_TOP: &[KeyHint] = &[
    ("Tab", "switch view"),
    ("Space", "select"),
    ("a", "all"),
    ("s", "all safe"),
    ("i", "invert"),
    ("n", "clear"),
];

const MEDIUM_BOT: &[KeyHint] = &[
    ("d", "move to Trash"),
    ("D", "delete forever"),
    ("r", "rescan"),
    ("?", "help"),
    ("q", "quit"),
];

const COMPACT_TOP: &[KeyHint] = &[
    ("Space", "sel"),
    ("a", "all"),
    ("s", "safe"),
    ("i", "inv"),
    ("n", "clr"),
];

const COMPACT_BOT: &[KeyHint] = &[
    ("d", "trash"),
    ("D", "del!"),
    ("r", "scan"),
    ("?", ""),
    ("q", ""),
];

fn key_legend(width: u16) -> Vec<Line<'static>> {
    let w = width as usize;
    if key_hint_width(FULL) <= w {
        vec![key_hint_line(FULL)]
    } else if key_hint_width(MEDIUM_TOP) <= w && key_hint_width(MEDIUM_BOT) <= w {
        vec![key_hint_line(MEDIUM_TOP), key_hint_line(MEDIUM_BOT)]
    } else {
        vec![key_hint_line(COMPACT_TOP), key_hint_line(COMPACT_BOT)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_when_wide_enough() {
        assert_eq!(key_legend(158).len(), 1);
    }

    #[test]
    fn two_lines_when_medium() {
        let lines = key_legend(100);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn compact_fits_narrow_widths() {
        assert!(key_hint_width(COMPACT_TOP) <= 46);
        assert!(key_hint_width(COMPACT_BOT) <= 46);
    }

    #[test]
    fn outer_height_grows_with_two_key_lines() {
        assert!(outer_height(160) < outer_height(100));
    }
}
