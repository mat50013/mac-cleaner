//! Dashboard bar chart overview.

use crate::fs_util::human_size;
use crate::model::{Category, ScanResults};
use crate::ui::theme;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Bar, BarChart, BarGroup};
use ratatui::Frame;

pub fn draw(f: &mut Frame, area: Rect, results: &ScanResults) {
    let max = Category::ALL
        .iter()
        .map(|c| results.total_bytes(*c))
        .max()
        .unwrap_or(1)
        .max(1);

    let bars: Vec<Bar> = Category::ALL
        .iter()
        .map(|cat| {
            let bytes = results.total_bytes(*cat);
            let label = format!("{} {}", cat.icon(), cat.title());
            let value = ((bytes as f64 / max as f64) * 100.0) as u64;
            Bar::default()
                .value(value.max(if bytes > 0 { 1 } else { 0 }))
                .label(Span::raw(format!("{label} {}", human_size(bytes))))
                .style(Style::default().fg(theme::ACCENT))
                .text_value(human_size(bytes))
        })
        .collect();

    let chart = BarChart::default()
        .block(theme::block(" Reclaimable by category "))
        .data(BarGroup::default().bars(&bars))
        .bar_width(3)
        .bar_gap(1)
        .value_style(Style::default().fg(Color::White));

    f.render_widget(chart, area);
}
