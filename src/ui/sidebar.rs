//! Category sidebar with Dashboard overview entry.
//!
//! Fixed-column layout (inner width 34):
//! marker(2) + bullet(2) + title(14) + count(7, right) + size(9, right).

use crate::fs_util::human_size;
use crate::model::{Category, MainView, ScanResults, ScanStatus};
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

const TITLE_W: usize = 14;
const COUNT_W: usize = 7;
const SIZE_W: usize = 9;

pub fn draw(f: &mut Frame, area: Rect, results: &ScanResults, view: MainView) {
    let mut items = vec![dashboard_row(results, matches!(view, MainView::Dashboard))];

    items.push(ListItem::new(Line::from(Span::styled(
        format!("  {}", "─".repeat(TITLE_W + COUNT_W + SIZE_W + 2)),
        theme::dim(),
    ))));

    for cat in Category::ALL {
        let is_current = matches!(view, MainView::Category(c) if c == cat);
        items.push(category_row(results, cat, is_current));
    }

    let list = List::new(items).block(theme::block(" Categories "));
    f.render_widget(list, area);
}

fn dashboard_row(results: &ScanResults, active: bool) -> ListItem<'static> {
    let selected: usize = results
        .items
        .values()
        .flat_map(|v| v.iter())
        .filter(|i| i.selected && i.selectable())
        .count();
    let total: usize = results.items.values().map(|v| v.len()).sum();

    let line = Line::from(vec![
        marker_span(active),
        Span::styled("◆ ", Style::default().fg(theme::accent())),
        Span::styled(format!("{:<TITLE_W$}", "Dashboard"), name_style(active)),
        count_span(selected, total),
        Span::styled(
            format!("{:>SIZE_W$}", human_size(results.total_reclaimable())),
            theme::text(),
        ),
    ]);
    row(line, active)
}

fn category_row(results: &ScanResults, cat: Category, active: bool) -> ListItem<'static> {
    let items = results.items_for(cat);
    let status = results.status.get(&cat);

    // While a scan is streaming, show live found counts instead of blanks.
    let (bytes, total, selected, pending) = match status {
        Some(ScanStatus::Scanning { found, bytes }) => (*bytes, *found, 0, false),
        Some(ScanStatus::Pending) => (0, 0, 0, true),
        _ => (
            items.iter().map(|i| i.real_bytes).sum(),
            items.len(),
            items
                .iter()
                .filter(|i| i.selected && i.selectable())
                .count(),
            false,
        ),
    };

    let empty = !pending && total == 0;
    let size_text = if pending {
        format!("{:>SIZE_W$}", "…")
    } else if empty {
        format!("{:>SIZE_W$}", "–")
    } else {
        format!("{:>SIZE_W$}", human_size(bytes))
    };
    let size_style = if empty { theme::dim() } else { theme::text() };

    let count = if empty {
        Span::styled(format!("{:>COUNT_W$}", "–"), theme::dim())
    } else {
        count_span(selected, total)
    };

    let line = Line::from(vec![
        marker_span(active),
        Span::styled("● ", Style::default().fg(theme::category_color(cat))),
        Span::styled(format!("{:<TITLE_W$}", cat.title()), name_style(active)),
        count,
        Span::styled(size_text, size_style),
    ]);
    row(line, active)
}

fn marker_span(active: bool) -> Span<'static> {
    if active {
        Span::styled("▸ ", Style::default().fg(theme::highlight()).bold())
    } else {
        Span::raw("  ")
    }
}

fn name_style(active: bool) -> Style {
    if active {
        Style::default().fg(theme::highlight()).bold()
    } else {
        theme::text()
    }
}

/// `sel/total`, highlighted when anything is selected; falls back to the
/// total alone if the pair would overflow its column.
fn count_span(selected: usize, total: usize) -> Span<'static> {
    let pair = format!("{selected}/{total}");
    let text = if pair.len() <= COUNT_W {
        format!("{pair:>COUNT_W$}")
    } else {
        format!("{total:>COUNT_W$}")
    };
    if selected > 0 {
        Span::styled(text, Style::default().fg(theme::safe()))
    } else {
        Span::styled(text, theme::dim())
    }
}

fn row(line: Line<'static>, active: bool) -> ListItem<'static> {
    let item = ListItem::new(line);
    if active {
        item.style(Style::default().bg(theme::selection_bg()))
    } else {
        item
    }
}
