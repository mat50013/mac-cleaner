//! Footer key legend and status.

use crate::model::ScanResults;
use crate::ui::theme;
use crate::ui::widgets::{format_selected, spinner_frame};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn draw(
    f: &mut Frame,
    area: Rect,
    results: &ScanResults,
    scanning: bool,
    tick: u64,
    status: &str,
    terminal_width: u16,
) {
    let selected = results.selected_items().len();
    let bytes = results.selected_bytes();
    let key_lines = key_legend(terminal_width);

    let mut lines = if scanning {
        vec![
            Line::from(vec![
                Span::styled(
                    format!("{} Scanning… ", spinner_frame(tick)),
                    Style::default().fg(theme::accent()),
                ),
                Span::styled(status, theme::dim()),
            ]),
        ]
    } else {
        vec![Line::from(vec![
            Span::styled(
                format_selected(selected, bytes),
                Style::default().fg(theme::safe()),
            ),
        ])]
    };
    lines.extend(key_lines);

    let block = theme::block("");
    f.render_widget(Paragraph::new(lines), block.inner(area));
    f.render_widget(block, area);
}

/// Key hints that fit the current terminal width (one or two lines).
fn key_legend(width: u16) -> Vec<Line<'static>> {
    if width >= 118 {
        vec![Line::from(
            "[Space] Select  [a] All in category  [A] Deselect  [s] Safe  [n] Clear  \
             [i] Invert  [d] Trash  [D] Delete forever  [r] Rescan  [?] Help  [q] Quit",
        )]
    } else if width >= 88 {
        vec![
            Line::from("[Space] [a/A] [s] [n] [i]  |  [d] Trash  [D] Forever  [r] Rescan  [?] [q]"),
        ]
    } else {
        vec![
            Line::from("[Space] [a] [s] [d] [D] [r] [?] [q]"),
            Line::from(Span::styled(
                "a=all  D=forever  r=rescan",
                theme::dim(),
            )),
        ]
    }
}
