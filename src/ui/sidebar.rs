//! Category sidebar with Dashboard overview entry.

use crate::fs_util::human_size;
use crate::model::{Category, MainView, ScanResults};
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

const DASHBOARD_ICON: &str = "\u{25C6}";

pub fn draw(f: &mut Frame, area: Rect, results: &ScanResults, view: MainView) {
    let dash_style = if matches!(view, MainView::Dashboard) {
        Style::default()
            .fg(theme::highlight())
            .add_modifier(Modifier::BOLD)
    } else {
        theme::text()
    };
    let total = results.total_reclaimable();
    let selected: usize = results
        .items
        .values()
        .flat_map(|v| v.iter())
        .filter(|i| i.selected)
        .count();
    let item_count: usize = results.items.values().map(|v| v.len()).sum();

    let mut items = vec![ListItem::new(Line::from(vec![
        Span::styled(format!("{DASHBOARD_ICON} "), dash_style),
        Span::styled("Dashboard", dash_style),
        Span::raw("  "),
        Span::styled(human_size(total), theme::dim()),
        Span::raw(format!("  {selected}/{item_count}")),
    ]))];

    for cat in Category::ALL {
        let bytes = results.total_bytes(cat);
        let cat_items = results.items_for(cat);
        let selected = cat_items.iter().filter(|i| i.selected).count();
        let total = cat_items.len();
        let is_current = matches!(view, MainView::Category(c) if c == cat);
        let style = if is_current {
            Style::default()
                .fg(theme::highlight())
                .add_modifier(Modifier::BOLD)
        } else {
            theme::text()
        };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{} ", cat.icon()), style),
            Span::styled(cat.title(), style),
            Span::raw("  "),
            Span::styled(human_size(bytes), theme::dim()),
            Span::raw(format!("  {selected}/{total}")),
        ])));
    }

    let list = List::new(items).block(theme::block(" Categories "));
    f.render_widget(list, area);
}
