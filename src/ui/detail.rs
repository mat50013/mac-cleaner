//! Detail table: the item list for one category.

use crate::fs_util::human_size;
use crate::model::{Category, ScanResults, ScanStatus};
use crate::ui::footer;
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, Table};

const BAR_WIDTH: usize = 12;

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

/// Number of data rows shown below the summary strip and the table header.
pub fn visible_data_rows(detail_panel: Rect) -> usize {
    let inner = theme::block("").inner(detail_panel);
    usize::from(inner.height.saturating_sub(2).max(1))
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
    let total_bytes: u64 = items.iter().map(|i| i.real_bytes).sum();

    let title = if items.is_empty() {
        format!(" {} ", category.title())
    } else {
        format!(
            " {} — {} · {} ",
            category.title(),
            count_label(items.len()),
            human_size(total_bytes)
        )
    };
    let block = theme::block(&title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if items.is_empty() {
        draw_empty_state(f, inner, results, category);
        return;
    }

    let strip_area = Rect { height: 1, ..inner };
    draw_summary_strip(f, strip_area, results, category, selected_row);

    let needs_scrollbar = items.len() > visible;
    let table_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner
            .width
            .saturating_sub(if needs_scrollbar { 1 } else { 0 }),
        height: inner.height.saturating_sub(1),
    };

    let max_bytes = items.iter().map(|i| i.real_bytes).max().unwrap_or(1).max(1);

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("Name"),
        Cell::from(Line::from("Size").alignment(Alignment::Right)),
        Cell::from(""),
        Cell::from("Risk"),
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
            let is_cursor = i == selected_row;
            let (check, check_style) = if !item.selectable() {
                ("[–]", theme::dim())
            } else if item.selected {
                ("[x]", Style::default().fg(theme::safe()).bold())
            } else {
                ("[ ]", theme::dim())
            };

            let tier_style = theme::tier_style(item.tier);
            let cells = vec![
                Cell::from(check).style(check_style),
                Cell::from(item.label.clone()).style(theme::text()),
                Cell::from(Line::from(human_size(item.real_bytes)).alignment(Alignment::Right))
                    .style(theme::text()),
                Cell::from(size_bar(item.real_bytes, max_bytes)).style(tier_style),
                Cell::from(item.tier.label()).style(tier_style),
                Cell::from(age_label(item.last_access_days)).style(theme::dim()),
                Cell::from(item.regen_note.clone()).style(theme::dim()),
            ];

            let row = Row::new(cells);
            if is_cursor {
                row.style(theme::selected_row_style())
            } else {
                row
            }
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Min(24),
        Constraint::Length(9),
        Constraint::Length(BAR_WIDTH as u16),
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Min(16),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(2);
    f.render_widget(table, table_area);

    if needs_scrollbar {
        let mut state = ratatui::widgets::ScrollbarState::new(items.len()).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let scrollbar_area = Rect {
            x: inner.x + inner.width.saturating_sub(1),
            y: table_area.y + 1,
            width: 1,
            height: table_area.height.saturating_sub(1),
        };
        f.render_stateful_widget(scrollbar, scrollbar_area, &mut state);
    }
}

/// One line above the table: selection summary on the left, cursor position on the right.
fn draw_summary_strip(
    f: &mut Frame,
    area: Rect,
    results: &ScanResults,
    category: Category,
    selected_row: usize,
) {
    let items = results.items_for(category);
    let sel_count = items
        .iter()
        .filter(|i| i.selected && i.selectable())
        .count();
    let sel_bytes: u64 = items
        .iter()
        .filter(|i| i.selected && i.selectable())
        .map(|i| i.real_bytes)
        .sum();

    let left = if sel_count > 0 {
        Line::from(Span::styled(
            format!(
                "{sel_count} of {} selected · {}",
                items.len(),
                human_size(sel_bytes)
            ),
            Style::default().fg(theme::safe()),
        ))
    } else {
        Line::from(Span::styled(
            "Nothing selected — Space toggles, a selects all",
            theme::dim(),
        ))
    };
    f.render_widget(Paragraph::new(left), area);

    let right = Line::from(Span::styled(
        format!(
            "{}/{}",
            selected_row.saturating_add(1).min(items.len()),
            items.len()
        ),
        theme::dim(),
    ));
    f.render_widget(Paragraph::new(right).alignment(Alignment::Right), area);
}

fn draw_empty_state(f: &mut Frame, inner: Rect, results: &ScanResults, category: Category) {
    let message: String = match results.status.get(&category) {
        Some(ScanStatus::Pending) | Some(ScanStatus::Scanning { .. }) => "Scanning…".to_string(),
        Some(ScanStatus::Skipped(reason)) => format!("Skipped: {reason}"),
        _ => "Nothing to clean here — this category is already tidy.".to_string(),
    };
    let y_offset = inner.height / 3;
    let area = Rect {
        x: inner.x,
        y: inner.y + y_offset,
        width: inner.width,
        height: inner.height.saturating_sub(y_offset).max(1),
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(message, theme::dim()))).centered(),
        area,
    );
}

/// Horizontal bar showing this item's size relative to the category's largest item.
fn size_bar(bytes: u64, max_bytes: u64) -> String {
    let ratio = bytes as f64 / max_bytes as f64;
    let filled = ((ratio * BAR_WIDTH as f64).round() as usize).clamp(1, BAR_WIDTH);
    let mut bar = "▮".repeat(filled);
    bar.push_str(&"▯".repeat(BAR_WIDTH - filled));
    bar
}

fn count_label(n: usize) -> String {
    if n == 1 {
        "1 item".to_string()
    } else {
        format!("{n} items")
    }
}

fn age_label(days: u32) -> String {
    match days {
        0 => "new".to_string(),
        1..=59 => format!("{days}d"),
        60..=364 => format!("{}mo", days / 30),
        _ => format!("{}y", days / 365),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_bar_scales_with_ratio() {
        assert_eq!(
            size_bar(100, 100).chars().filter(|c| *c == '▮').count(),
            BAR_WIDTH
        );
        assert_eq!(
            size_bar(50, 100).chars().filter(|c| *c == '▮').count(),
            BAR_WIDTH / 2
        );
        // Even tiny items get one visible cell.
        assert_eq!(
            size_bar(1, 1_000_000).chars().filter(|c| *c == '▮').count(),
            1
        );
        assert_eq!(size_bar(1, 100).chars().count(), BAR_WIDTH);
    }

    #[test]
    fn age_label_buckets() {
        assert_eq!(age_label(0), "new");
        assert_eq!(age_label(12), "12d");
        assert_eq!(age_label(90), "3mo");
        assert_eq!(age_label(800), "2y");
    }

    #[test]
    fn count_label_handles_singular() {
        assert_eq!(count_label(1), "1 item");
        assert_eq!(count_label(3), "3 items");
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
