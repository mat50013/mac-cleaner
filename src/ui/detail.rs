//! Detail table with checkboxes.

use crate::fs_util::human_size;
use crate::model::{Category, ScanResults};
use crate::ui::theme;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, Table};
use ratatui::Frame;

pub fn draw(
    f: &mut Frame,
    area: Rect,
    results: &ScanResults,
    category: Category,
    selected_row: usize,
    scroll: usize,
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
        .take(table_area.height.saturating_sub(2) as usize)
        .map(|(i, item)| {
            let global_i = i + scroll;
            let check = if !item.selectable() {
                "[—]"
            } else if item.selected {
                "[x]"
            } else {
                "[ ]"
            };
            let row_style = if global_i == selected_row {
                Style::default().bg(theme::surface()).add_modifier(Modifier::BOLD)
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

    // Draw size bars over the bar column (approximate overlay).
    // Simpler: size is in its own column; bars shown in label truncation.

    if items.len() > table_area.height as usize {
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
}
