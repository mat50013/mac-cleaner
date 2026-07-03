//! TUI rendering entry point.

pub mod dashboard;
pub mod detail;
pub mod footer;
pub mod header;
pub mod modal;
pub mod sidebar;
pub mod terminal;
pub mod theme;
pub mod widgets;

use crate::app::App;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use terminal::{is_large_enough, MIN_HEIGHT, MIN_WIDTH, PREFERRED_HEIGHT, PREFERRED_WIDTH};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    if !is_large_enough(area.width, area.height) {
        draw_too_small(f, area);
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(4),
    ])
    .split(area);

    header::draw(
        f,
        chunks[0],
        app.disk,
        app.results.total_reclaimable(),
        app.privilege.limited,
    );

    let body = Layout::horizontal([Constraint::Length(36), Constraint::Min(20)]).split(chunks[1]);

    if app.show_dashboard && !app.scanning {
        dashboard::draw(f, body[1], &app.results);
    } else {
        detail::draw(
            f,
            body[1],
            &app.results,
            app.current_category,
            app.selected_row,
            app.scroll,
        );
    }

    sidebar::draw(f, body[0], &app.results, app.current_category);

    footer::draw(
        f,
        chunks[2],
        &app.results,
        app.scanning,
        app.tick,
        &app.status_line,
        area.width,
    );

    modal::draw(f, area, &app.modal);
}

fn draw_too_small(f: &mut Frame, area: ratatui::layout::Rect) {
    use crate::ui::theme;
    let msg = vec![
        Line::from(Span::styled("Terminal too small", theme::title_style())),
        Line::from(""),
        Line::from(format!(
            "Current size: {}×{}   Need at least: {}×{}",
            area.width, area.height, MIN_WIDTH, MIN_HEIGHT
        )),
        Line::from(format!(
            "Recommended: {}×{} (drag the window corner or use a larger profile)",
            PREFERRED_WIDTH, PREFERRED_HEIGHT
        )),
        Line::from(""),
        Line::from(Span::styled("Press [q] to quit", theme::text())),
    ];
    let para = Paragraph::new(msg).centered();
    f.render_widget(para, area);
}
