//! TUI rendering entry point.

pub mod dashboard;
pub mod detail;
pub mod footer;
pub mod header;
pub mod modal;
pub mod sidebar;
pub mod theme;
pub mod widgets;

use crate::app::App;
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(3),
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
    );

    modal::draw(f, area, &app.modal);
}
