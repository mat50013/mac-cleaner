//! Header: title, disk gauge, reclaimable total.

use crate::event::DiskInfo;
use crate::fs_util::human_size;
use crate::ui::theme;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{LineGauge, Paragraph};
use ratatui::Frame;

const LEFT_W: u16 = 22;
const MIN_GAUGE_W: u16 = 10;

pub fn draw(f: &mut Frame, area: Rect, disk: DiskInfo, reclaimable: u64, limited: bool) {
    let chunks = Layout::horizontal([
        Constraint::Length(LEFT_W.min(area.width)),
        Constraint::Min(MIN_GAUGE_W + 8),
    ])
    .split(area);

    let title = if limited {
        " mac-cleaner  [limited] "
    } else {
        " mac-cleaner "
    };
    let block = theme::block(title);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Reclaimable: ", theme::dim()),
            Span::styled(
                human_size(reclaimable),
                Style::default().fg(theme::safe()).bold(),
            ),
        ])),
        block.inner(chunks[0]),
    );
    f.render_widget(block, chunks[0]);

    // Hints live on the same row as the disk gauge, inside the Disk block.
    let disk_block = theme::block(" Disk ");
    let disk_outer = chunks[1];
    let disk_inner = disk_block.inner(disk_outer);
    f.render_widget(disk_block, disk_outer);

    let hint_room = disk_inner
        .width
        .saturating_sub(MIN_GAUGE_W)
        .saturating_sub(1);
    let hint = hint_text(hint_room);
    let hint_w = hint.len() as u16;

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

        let hint_para = Paragraph::new(Line::from(Span::styled(hint, theme::text())))
            .alignment(Alignment::Right);
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
fn hint_text(hint_room: u16) -> &'static str {
    if hint_room >= 24 {
        "[?] Help  |  [r] Rescan"
    } else if hint_room >= 18 {
        "[?] Help  [r] Rescan"
    } else if hint_room >= 8 {
        "[?]  [r]"
    } else {
        ""
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
        assert!(hint_text(24).contains("Rescan"));
        assert!(hint_text(10).len() <= 10);
        assert_eq!(hint_text(4), "");
    }
}
