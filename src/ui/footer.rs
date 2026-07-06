//! Footer key legend and status.

use crate::model::ScanResults;
use crate::ui::theme;
use crate::ui::widgets::{format_selected, spinner_frame};
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
    let key_lines = key_legend(inner.width);

    let mut lines = if scanning {
        vec![Line::from(vec![
            Span::styled(
                format!("{} Scanning… ", spinner_frame(tick)),
                Style::default().fg(theme::accent()),
            ),
            Span::styled(status, theme::dim()),
        ])]
    } else {
        vec![Line::from(vec![Span::styled(
            format_selected(results.selected_items().len(), results.selected_bytes()),
            Style::default().fg(theme::safe()),
        )])]
    };
    lines.extend(key_lines);

    f.render_widget(block, area);
    f.render_widget(Paragraph::new(lines), inner);
}

fn inner_width(terminal_width: u16) -> u16 {
    terminal_width.saturating_sub(2)
}

fn key_legend(width: u16) -> Vec<Line<'static>> {
    const ONE: &str = "[Space] Select  [a] All  [A] Desel  [s] Safe  [n] Clear  [i] Inv  [d] Trash  [D] Del  [r] Scan  [?] Help  [q] Quit";
    const TWO_TOP: &str =
        "[Space] Select  [a] All in category  [A] Deselect  [s] Safe  [n] Clear  [i] Invert";
    const TWO_BOT: &str = "[d] Trash  [D] Delete forever  [r] Rescan  [?] Help  [q] Quit";
    const COMPACT_TOP: &str = "[Space] [a/A] [s] [n] [i]";
    const COMPACT_BOT: &str = "[d] Trash  [D] Forever  [r] Rescan  [?] [q]";

    let w = width as usize;
    if fits(ONE, w) {
        vec![styled_keys(ONE)]
    } else if fits(TWO_TOP, w) && fits(TWO_BOT, w) {
        vec![styled_keys(TWO_TOP), styled_keys(TWO_BOT)]
    } else {
        vec![styled_keys(COMPACT_TOP), styled_keys(COMPACT_BOT)]
    }
}

fn fits(text: &str, width: usize) -> bool {
    text.len() <= width
}

fn styled_keys(text: &'static str) -> Line<'static> {
    Line::from(Span::styled(text, theme::text()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_when_wide_enough() {
        assert_eq!(key_legend(118).len(), 1);
    }

    #[test]
    fn two_lines_when_medium() {
        let lines = key_legend(90);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn outer_height_grows_with_two_key_lines() {
        assert!(outer_height(120) < outer_height(80));
    }
}
