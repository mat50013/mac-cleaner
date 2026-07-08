//! Dashboard pie chart: reclaimable space per category.

use crate::fs_util::human_size;
use crate::model::{Category, ScanResults};
use crate::ui::theme;
use crate::ui::widgets::key_hint_line;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::f64::consts::TAU;

struct Slice {
    category: Category,
    bytes: u64,
    color: Color,
    start: f64,
    end: f64,
}

pub fn draw(f: &mut Frame, area: Rect, results: &ScanResults, scanning: bool) {
    let block = theme::block(" Dashboard — reclaimable space ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 4 {
        return;
    }

    let total = results.total_reclaimable();
    if total == 0 {
        let text = if scanning {
            "Scanning your Mac — results appear here as categories finish."
        } else {
            "No reclaimable space found yet — press r to rescan."
        };
        let msg = Paragraph::new(Line::from(Span::styled(text, theme::dim()))).centered();
        let mid = Rect {
            y: inner.y + inner.height / 2,
            height: 1,
            ..inner
        };
        f.render_widget(msg, mid);
        return;
    }

    let slices = build_slices(results, total);
    if slices.is_empty() {
        return;
    }

    let chart_area = Rect {
        height: inner.height.saturating_sub(2),
        ..inner
    };
    let cols =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Min(22)]).split(chart_area);

    draw_pie(f.buffer_mut(), cols[0], &slices, total);
    draw_legend(f, cols[1], &slices, results, total);

    let hint_area = Rect {
        y: inner.y + inner.height.saturating_sub(1),
        height: 1,
        ..inner
    };
    let hint = key_hint_line(&[
        ("Tab", "open a category"),
        ("s", "select everything safe"),
        ("d", "clean"),
    ]);
    f.render_widget(Paragraph::new(hint).centered(), hint_area);
}

fn build_slices(results: &ScanResults, total: u64) -> Vec<Slice> {
    let mut slices = Vec::new();
    let mut angle = 0.0f64;

    for cat in Category::ALL {
        let bytes = results.total_bytes(cat);
        if bytes == 0 {
            continue;
        }
        let sweep = (bytes as f64 / total as f64) * TAU;
        slices.push(Slice {
            category: cat,
            bytes,
            color: theme::category_color(cat),
            start: angle,
            end: angle + sweep,
        });
        angle += sweep;
    }

    slices
}

/// Draw the donut chart cell by cell.
fn draw_pie(buf: &mut Buffer, area: Rect, slices: &[Slice], total: u64) {
    if area.width < 3 || area.height < 3 {
        return;
    }

    let cx = area.x as f64 + (area.width as f64 - 1.0) / 2.0;
    let cy = area.y as f64 + (area.height as f64 - 1.0) / 2.0;
    let radius = (area.width as f64 / 2.0 - 1.0)
        .min(area.height as f64 - 1.0)
        .max(2.0);
    let inner_r = radius * 0.42;
    let inner_r2 = inner_r * inner_r;
    let outer_r2 = radius * radius;

    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let dx = x as f64 - cx;
            let dy = (y as f64 - cy) * 2.0;
            let dist2 = dx * dx + dy * dy;
            if dist2 < inner_r2 || dist2 > outer_r2 {
                continue;
            }

            let mut angle = dx.atan2(-dy);
            if angle < 0.0 {
                angle += TAU;
            }

            if let Some(slice) = slices.iter().find(|s| angle >= s.start && angle < s.end) {
                buf[(x, y)]
                    .set_symbol("█")
                    .set_style(Style::default().fg(slice.color).bg(theme::surface()));
            }
        }
    }

    let label_w = human_size(total).len() as u16 + 2;
    let label_h = 3u16;
    let lx = cx as u16 - label_w / 2;
    let ly = cy as u16 - label_h / 2;
    let label_area = Rect {
        x: lx.max(area.x),
        y: ly.max(area.y),
        width: label_w.min(area.width),
        height: label_h.min(area.height),
    };
    let center = Paragraph::new(vec![
        Line::from(Span::styled("Total", theme::dim())),
        Line::from(Span::styled(human_size(total), theme::title_style())),
    ])
    .centered();
    center.render(label_area, buf);
}

fn draw_legend(f: &mut Frame, area: Rect, slices: &[Slice], results: &ScanResults, total: u64) {
    let mut lines: Vec<Line> = Vec::with_capacity(slices.len() + 2);
    lines.push(Line::from(Span::styled("Breakdown", theme::title_style())));
    lines.push(Line::from(""));

    for slice in slices {
        let pct = slice.bytes as f64 / total as f64 * 100.0;
        let count = results.items_for(slice.category).len();
        let count_text = if count == 1 {
            "  1 item".to_string()
        } else {
            format!("  {count} items")
        };
        lines.push(Line::from(vec![
            Span::styled("██ ", Style::default().fg(slice.color)),
            Span::styled(format!("{:<14}", slice.category.title()), theme::text()),
            Span::styled(
                format!("{:>9}", human_size(slice.bytes)),
                Style::default().fg(slice.color),
            ),
            Span::styled(format!("{:>5.0}%", pct), theme::dim()),
            Span::styled(count_text, theme::dim()),
        ]));
    }

    // Vertically center the legend beside the pie.
    let legend_h = lines.len() as u16;
    let y_offset = area.height.saturating_sub(legend_h) / 2;
    let centered = Rect {
        y: area.y + y_offset,
        height: area.height.saturating_sub(y_offset),
        ..area
    };
    f.render_widget(Paragraph::new(lines), centered);
}
