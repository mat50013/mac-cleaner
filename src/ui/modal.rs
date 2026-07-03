//! Modal overlays: confirm, progress, help, FDA, Docker.

use crate::fs_util::human_size;
use crate::ui::theme;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    Help,
    ConfirmClean { count: usize, bytes: u64 },
    CleanProgress { done: usize, total: usize, freed: u64 },
    CleanDone { freed: u64, failures: Vec<String> },
    Fda,
    DockerStart,
}

pub fn draw(f: &mut Frame, area: Rect, modal: &Modal) {
    match modal {
        Modal::None => {}
        Modal::Help => draw_help(f, area),
        Modal::ConfirmClean { count, bytes } => {
            draw_centered(
                f,
                area,
                "Confirm clean",
                &format!(
                    "Delete {count} items ({}) to Trash?\n\n[y] Yes  [n] No",
                    human_size(*bytes)
                ),
            );
        }
        Modal::CleanProgress { done, total, freed } => {
            draw_centered(
                f,
                area,
                "Cleaning…",
                &format!(
                    "Progress: {done}/{total}\nFreed so far: {}\n\nPlease wait…",
                    human_size(*freed)
                ),
            );
        }
        Modal::CleanDone { freed, failures } => {
            let mut body = format!("Freed {}.", human_size(*freed));
            if !failures.is_empty() {
                body.push_str(&format!("\n\n{} errors:\n", failures.len()));
                for e in failures.iter().take(5) {
                    body.push_str(&format!("• {e}\n"));
                }
            }
            body.push_str("\n[Enter] Close");
            draw_centered(f, area, "Done", &body);
        }
        Modal::Fda => draw_centered(
            f,
            area,
            "Full Disk Access",
            "Some paths require Full Disk Access.\n\n\
             1. Open System Settings → Privacy & Security\n\
             2. Full Disk Access → add Terminal (or mac-cleaner)\n\n\
             [o] Open Settings  [n] Continue without",
        ),
        Modal::DockerStart => draw_centered(
            f,
            area,
            "Docker",
            "Docker is not running.\nStart Docker to reclaim build cache and images?\n\n\
             [y] Start Docker  [n] Skip",
        ),
    }
}

fn draw_centered(f: &mut Frame, area: Rect, title: &str, body: &str) {
    let popup = centered_rect(60, 40, area);
    f.render_widget(Clear, popup);
    let block = theme::block(title);
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let para = Paragraph::new(body)
        .wrap(Wrap { trim: true })
        .style(theme::text());
    f.render_widget(para, inner);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let text = "\
Navigation\n  ↑/↓ or j/k  move selection\n  Tab / Shift+Tab  switch category\n\n\
Selection\n  Space       toggle item\n  a           select ALL in category\n  A           deselect category\n  s           select all Safe items\n  n           clear all selections\n  i           invert category selection\n  Enter       flip duplicate keeper\n\n\
Actions\n  d           clean selected items\n  r           rescan\n  ?           this help\n  q / Esc     quit\n";
    draw_centered(f, area, "Help", text);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
