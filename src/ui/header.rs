//! Header: title, totals, and disk gauge.

use crate::event::DiskInfo;
use crate::fs_util::human_size;
use crate::ui::theme;
use crate::ui::widgets::{key_hint_line, key_hint_width};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{LineGauge, Paragraph};

const LEFT_W: u16 = 46;
const MIN_GAUGE_W: u16 = 10;

pub fn draw(
    f: &mut Frame,
    area: Rect,
    disk: DiskInfo,
    reclaimable: u64,
    selected: u64,
    limited: bool,
) {
    let chunks = Layout::horizontal([
        Constraint::Length(LEFT_W.min(area.width)),
        Constraint::Min(MIN_GAUGE_W + 8),
    ])
    .split(area);

    let title = if limited {
        concat!(" mac-cleaner v", env!("CARGO_PKG_VERSION"), " — limited ")
    } else {
        concat!(" mac-cleaner v", env!("CARGO_PKG_VERSION"), " ")
    };
    let block = theme::block(title);
    let mut totals = vec![
        Span::styled("Reclaimable ", theme::dim()),
        Span::styled(
            human_size(reclaimable),
            Style::default().fg(theme::safe()).bold(),
        ),
    ];
    if selected > 0 {
        totals.push(Span::styled("  ·  Selected ", theme::dim()));
        totals.push(Span::styled(
            human_size(selected),
            Style::default().fg(theme::accent()).bold(),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(totals)), block.inner(chunks[0]));
    f.render_widget(block, chunks[0]);

    let disk_block = theme::block(" Disk ");
    let disk_outer = chunks[1];
    let disk_inner = disk_block.inner(disk_outer);
    f.render_widget(disk_block, disk_outer);

    let hint_room = disk_inner
        .width
        .saturating_sub(MIN_GAUGE_W)
        .saturating_sub(1);
    let hints = hint_pairs(hint_room);
    let hint_w = key_hint_width(hints) as u16;

    if hint_w > 0 && disk_inner.width > MIN_GAUGE_W + hint_w {
        let cols = Layout::horizontal([
            Constraint::Min(MIN_GAUGE_W),
            Constraint::Length(2),
            Constraint::Length(hint_w),
        ])
        .split(disk_inner);

        let gauge = LineGauge::default()
            .filled_style(Style::default().fg(theme::accent()))
            .filled_symbol("█")
            .unfilled_symbol("░")
            .label(disk_label(disk, cols[0].width))
            .ratio(disk.used_ratio());
        f.render_widget(gauge, cols[0]);

        let hint_para = Paragraph::new(key_hint_line(hints)).alignment(Alignment::Right);
        f.render_widget(hint_para, cols[2]);
    } else {
        let gauge = LineGauge::default()
            .filled_style(Style::default().fg(theme::accent()))
            .filled_symbol("█")
            .unfilled_symbol("░")
            .label(disk_label(disk, disk_inner.width))
            .ratio(disk.used_ratio());
        f.render_widget(gauge, disk_inner);
    }
}

/// Shortcut hints shown beside the disk gauge (same row).
fn hint_pairs(hint_room: u16) -> &'static [(&'static str, &'static str)] {
    if hint_room >= 26 {
        &[("?", "help"), ("r", "rescan"), ("q", "quit")]
    } else if hint_room >= 17 {
        &[("?", "help"), ("r", "rescan")]
    } else if hint_room >= 5 {
        &[("?", ""), ("r", "")]
    } else {
        &[]
    }
}

/// Shorter disk label when the gauge column is narrow.
fn disk_label(disk: DiskInfo, width: u16) -> String {
    if width >= 36 {
        format!(
            "{} used / {} free",
            human_size(disk.used()),
            human_size(disk.free)
        )
    } else if width >= 22 {
        format!("{} / {}", human_size(disk.used()), human_size(disk.free))
    } else {
        format!("{}%", (disk.used_ratio() * 100.0) as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_scales_with_room() {
        for room in [40u16, 26, 20, 17, 8, 5] {
            assert!(
                key_hint_width(hint_pairs(room)) <= room as usize,
                "hints overflow at room {room}"
            );
        }
        assert!(!hint_pairs(26).is_empty());
        assert!(hint_pairs(3).is_empty());
    }
}
