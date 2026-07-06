//! Detail table.

use crate::fs_util::human_size;
use crate::model::{Category, ScanResults};
use crate::ui::footer;
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, Table};

/// Detail panel rectangle for a given terminal size.
pub fn panel_rect(terminal_width: u16, terminal_height: u16) -> Rect {
    let footer_h = footer::outer_height(terminal_width);
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(footer_h),
    ])
    .split(Rect {
        x: 0,
        y: 0,
        width: terminal_width,
        height: terminal_height,
    });
    Layout::horizontal([Constraint::Length(36), Constraint::Min(20)]).split(chunks[1])[1]
}

/// Layout metrics derived from the actual draw area.
pub struct DetailMetrics {
    pub visible_data_rows: usize,
}

pub fn metrics(detail_panel: Rect) -> DetailMetrics {
    DetailMetrics {
        visible_data_rows: visible_data_rows(detail_panel),
    }
}

/// Number of data rows shown below the table header.
pub fn visible_data_rows(detail_panel: Rect) -> usize {
    let block = theme::block("");
    let inner = block.inner(detail_panel);
    let banner_h = selection_hints(inner.width).len() as u16;
    let table_h = inner.height.saturating_sub(banner_h);
    usize::from(table_h.saturating_sub(1).max(1))
}

pub fn visible_data_rows_for_terminal(terminal_width: u16, terminal_height: u16) -> usize {
    visible_data_rows(panel_rect(terminal_width, terminal_height))
}

pub fn draw(
    f: &mut Frame,
    area: Rect,
    results: &ScanResults,
    category: Category,
    selected_row: usize,
    scroll: usize,
    visible: usize,
) {
    let items = results.items_for(category);
    let _max_bytes = items.iter().map(|i| i.real_bytes).max().unwrap_or(1);

    let title = format!(" {} ", category.title());
    let block = theme::block(&title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let banner_lines = selection_hints(inner.width);
    let banner_h = banner_lines.len() as u16;

    let banner_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: banner_h,
    };
    f.render_widget(Paragraph::new(banner_lines), banner_area);

    let table_area = Rect {
        x: inner.x,
        y: inner.y + banner_h,
        width: inner.width,
        height: inner.height.saturating_sub(banner_h),
    };

    let header = Row::new(vec![
        Cell::from("[x]"),
        Cell::from("Label"),
        Cell::from("Size"),
        Cell::from("Bar"),
        Cell::from("Tier"),
        Cell::from("Age"),
        Cell::from("Note"),
    ])
    .style(theme::title_style());

    let rows: Vec<Row> = items
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(i, item)| {
            let check = if !item.selectable() {
                "[—]"
            } else if item.selected {
                "[x]"
            } else {
                "[ ]"
            };
            let row_style = if i == selected_row {
                Style::default()
                    .bg(theme::surface())
                    .add_modifier(Modifier::BOLD)
            } else {
                theme::tier_style(item.tier)
            };
            Row::new(vec![
                Cell::from(check),
                Cell::from(item.label.clone()),
                Cell::from(human_size(item.real_bytes)),
                Cell::from("").style(row_style),
                Cell::from(item.tier.label()),
                Cell::from(format!("{}d", item.last_access_days)),
                Cell::from(item.regen_note.clone()),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Min(20),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(5),
        Constraint::Min(12),
    ];

    let table = Table::new(rows, widths).header(header);
    f.render_widget(table, table_area);

    if items.len() > visible {
        let mut state = ratatui::widgets::ScrollbarState::new(items.len()).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        f.render_stateful_widget(scrollbar, table_area, &mut state);
    }
}

/// Selection hints above the table — one or two lines depending on width.
fn selection_hints(width: u16) -> Vec<Line<'static>> {
    const ONE: &str = "[a] All in category  [s] Safe  [Space] toggle";
    const TWO_TOP: &str = "[a] All in category  [s] All Safe items";
    const TWO_BOT: &str = "[Space] toggle selection";
    const COMPACT: &str = "[a] all  [s] safe  [Space]";

    let w = width as usize;
    let dim = theme::dim();
    if fits(ONE, w) {
        vec![Line::from(Span::styled(ONE, dim))]
    } else if fits(TWO_TOP, w) && fits(TWO_BOT, w) {
        vec![
            Line::from(Span::styled(TWO_TOP, dim)),
            Line::from(Span::styled(TWO_BOT, dim)),
        ]
    } else {
        vec![Line::from(Span::styled(COMPACT, dim))]
    }
}

fn fits(text: &str, width: usize) -> bool {
    text.len() <= width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_hints_one_line_when_wide() {
        assert_eq!(selection_hints(80).len(), 1);
    }

    #[test]
    fn selection_hints_two_lines_when_medium() {
        // ONE is 45 chars; between 39 and 44 only the two-line split fits.
        assert_eq!(selection_hints(42).len(), 2);
    }

    #[test]
    fn compact_hints_fit_narrow_panels() {
        let line = &selection_hints(30)[0];
        assert!(line.spans[0].content.len() <= 30);
    }

    #[test]
    fn visible_data_rows_is_at_least_one() {
        let panel = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 20,
        };
        assert!(visible_data_rows(panel) >= 1);
    }

    #[test]
    fn taller_panel_shows_more_rows() {
        let short = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 14,
        };
        let tall = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        assert!(visible_data_rows(tall) > visible_data_rows(short));
    }
}
