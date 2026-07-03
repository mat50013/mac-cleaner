//! Header: title, disk gauge, reclaimable total.

use crate::event::DiskInfo;
use crate::fs_util::human_size;
use crate::ui::theme;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{LineGauge, Paragraph};
use ratatui::Frame;

pub fn draw(f: &mut Frame, area: Rect, disk: DiskInfo, reclaimable: u64, limited: bool) {
    let chunks = Layout::horizontal([
        Constraint::Length(22),
        Constraint::Min(20),
        Constraint::Length(24),
    ])
    .split(area);

    let title = if limited {
        " mac-cleaner  [limited mode] "
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

    let _used_pct = (disk.used_ratio() * 100.0) as u16;
    let gauge = LineGauge::default()
        .block(theme::block(" Disk "))
        .filled_style(Style::default().fg(theme::accent()))
        .filled_symbol("█")
        .unfilled_symbol("░")
        .label(format!(
            "{} used / {} free",
            human_size(disk.used()),
            human_size(disk.free)
        ))
        .ratio(disk.used_ratio());
    f.render_widget(gauge, chunks[1]);

    let hint = Paragraph::new(Line::from(Span::styled(
        "Press [?] for help  |  [r] Rescan",
        theme::text(),
    )));
    f.render_widget(hint, chunks[2]);
}
