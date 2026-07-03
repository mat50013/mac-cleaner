//! Footer key legend and status.

use crate::model::ScanResults;
use crate::ui::theme;
use crate::ui::widgets::{format_selected, spinner_frame};
use ratatui::layout::Rect;
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
) {
    let selected = results.selected_items().len();
    let bytes = results.selected_bytes();

    let keys = "[Space] Select  [a] Select all in category  [A] Deselect category  \
                  [s] Select all safe  [n] Clear  [i] Invert  [d] Clean  [?] Help  [q] Quit";

    let line = if scanning {
        Line::from(vec![
            Span::styled(format!("{} Scanning… ", spinner_frame(tick)), theme::ACCENT),
            Span::styled(status, theme::dim()),
            Span::raw("  |  "),
            Span::styled(keys, theme::dim()),
        ])
    } else {
        Line::from(vec![
            Span::styled(format_selected(selected, bytes), theme::SAFE),
            Span::raw("  |  "),
            Span::styled(keys, theme::dim()),
        ])
    };

    let block = theme::block("");
    f.render_widget(Paragraph::new(line), block.inner(area));
    f.render_widget(block, area);
}
