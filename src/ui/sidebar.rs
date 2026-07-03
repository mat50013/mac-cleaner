//! Category sidebar.

use crate::fs_util::human_size;
use crate::model::{Category, ScanResults};
use crate::ui::theme;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};
use ratatui::Frame;

pub fn draw(f: &mut Frame, area: Rect, results: &ScanResults, current: Category) {
    let items: Vec<ListItem> = Category::ALL
        .iter()
        .map(|cat| {
            let bytes = results.total_bytes(*cat);
            let items = results.items_for(*cat);
            let selected = items.iter().filter(|i| i.selected).count();
            let total = items.len();
            let is_current = *cat == current;
            let style = if is_current {
                Style::default()
                    .fg(theme::highlight())
                    .add_modifier(Modifier::BOLD)
            } else {
                theme::text()
            };
            let hint = if is_current {
                "  press a: select all"
            } else {
                ""
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", cat.icon()), style),
                Span::styled(cat.title(), style),
                Span::raw("  "),
                Span::styled(human_size(bytes), theme::dim()),
                Span::raw(format!("  {selected}/{total}")),
                Span::styled(hint, theme::dim()),
            ]))
        })
        .collect();

    let list = List::new(items).block(theme::block(" Categories "));
    f.render_widget(list, area);
}
