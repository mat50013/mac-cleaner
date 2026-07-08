//! Modal overlays: confirm, progress, help, FDA, Docker.

use crate::fs_util::human_size;
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    Help,
    ConfirmClean {
        count: usize,
        bytes: u64,
        permanent: bool,
        empty_trash: bool,
        evict_only: bool,
    },
    CleanProgress {
        done: usize,
        total: usize,
        freed: u64,
    },
    CleanDone {
        freed: u64,
        failures: Vec<String>,
    },
    Fda,
    DockerStart,
}

pub fn draw(f: &mut Frame, area: Rect, modal: &Modal) {
    match modal {
        Modal::None => {}
        Modal::Help => draw_help(f, area),
        Modal::ConfirmClean {
            count,
            bytes,
            permanent,
            empty_trash,
            evict_only,
        } => {
            let body = if *empty_trash {
                format!(
                    "Permanently empty Trash ({})?\n\nThis cannot be undone.\n\n[y] Yes  [n] No",
                    human_size(*bytes)
                )
            } else if *evict_only {
                format!(
                    "Evict local copy of {count} items ({})?\n\n\
                     Files stay in iCloud — re-download from Finder anytime.\n\n\
                     [y] Yes  [n] No",
                    human_size(*bytes)
                )
            } else if *permanent {
                format!(
                    "Permanently delete {count} items ({})?\n\nThis cannot be undone.\n\n[y] Yes  [n] No",
                    human_size(*bytes)
                )
            } else {
                format!(
                    "Move {count} items ({}) to Trash?\n\nUse [D] to delete forever instead.\n\n[y] Yes  [n] No",
                    human_size(*bytes)
                )
            };
            draw_centered(f, area, "Confirm clean", &body);
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
    draw_centered_sized(f, area, title, body, 60, 40);
}

fn draw_centered_sized(f: &mut Frame, area: Rect, title: &str, body: &str, pct_x: u16, pct_y: u16) {
    let popup = centered_rect(pct_x, pct_y, area);
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
    use crate::model::SafetyTier;

    let mut lines: Vec<Line<'static>> = Vec::new();

    let section = |lines: &mut Vec<Line<'static>>, name: &'static str| {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(name, theme::title_style())));
    };
    let entry = |lines: &mut Vec<Line<'static>>, key: &'static str, desc: &'static str| {
        lines.push(Line::from(vec![
            Span::styled(format!("  {key:<18}"), theme::key_style()),
            Span::styled(desc, theme::text()),
        ]));
    };

    section(&mut lines, "Navigate");
    entry(
        &mut lines,
        "Tab / Shift+Tab",
        "next / previous view — Dashboard first",
    );
    entry(&mut lines, "↑ ↓  or  j k", "move between rows");

    section(&mut lines, "Select");
    entry(&mut lines, "Space", "toggle the highlighted item");
    entry(
        &mut lines,
        "a / A",
        "select / deselect everything in this category",
    );
    entry(
        &mut lines,
        "s",
        "select every Safe item across all categories",
    );
    entry(&mut lines, "i", "invert this category's selection");
    entry(&mut lines, "n", "clear all selections");
    entry(&mut lines, "Enter", "Duplicates: choose which copy to keep");
    entry(&mut lines, "", "Caches: start Docker when prompted");

    section(&mut lines, "Clean");
    entry(
        &mut lines,
        "d",
        "move selected items to the Trash (undo with Put Back)",
    );
    entry(
        &mut lines,
        "D",
        "delete selected items forever — cannot be undone",
    );
    entry(&mut lines, "r", "rescan (do this after cleaning)");

    section(&mut lines, "Risk colors");
    let tier = |lines: &mut Vec<Line<'static>>, t: SafetyTier, desc: &'static str| {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18}", t.label()), theme::tier_style(t)),
            Span::styled(desc, theme::text()),
        ]));
    };
    tier(
        &mut lines,
        SafetyTier::Safe,
        "regenerable junk — selected automatically",
    );
    tier(
        &mut lines,
        SafetyTier::Moderate,
        "review first — may cost a rebuild or re-download",
    );
    tier(
        &mut lines,
        SafetyTier::Risky,
        "user data or duplicate keeper — never auto-selected",
    );

    section(&mut lines, "Notes");
    lines.push(Line::from(Span::styled(
        "  Trash Bin items are always removed permanently, even with d.",
        theme::dim(),
    )));
    lines.push(Line::from(Span::styled(
        "  After cleaning with d, rescan and then empty the Trash Bin.",
        theme::dim(),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press any key to close this help · q or Esc quits the app",
        theme::dim(),
    )));

    draw_popup_sized(f, area, " Help ", lines, 72, 82);
}

fn draw_popup_sized(
    f: &mut Frame,
    area: Rect,
    title: &str,
    body: Vec<Line<'static>>,
    pct_x: u16,
    pct_y: u16,
) {
    f.render_widget(Block::default().style(theme::modal_backdrop()), area);

    let popup = centered_rect(pct_x, pct_y, area);

    const SHADOW_DX: u16 = 2;
    const SHADOW_DY: u16 = 1;
    let shadow = offset_rect(popup, SHADOW_DX, SHADOW_DY);
    f.render_widget(
        Block::default().style(theme::modal_shadow()),
        clip_rect(shadow, area),
    );

    f.render_widget(Clear, popup);
    let block = theme::modal_block(title);
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let para = Paragraph::new(body)
        .wrap(Wrap { trim: true })
        .style(theme::text());
    f.render_widget(para, inner);
}

fn offset_rect(r: Rect, dx: u16, dy: u16) -> Rect {
    Rect {
        x: r.x.saturating_add(dx),
        y: r.y.saturating_add(dy),
        width: r.width,
        height: r.height,
    }
}

fn clip_rect(inner: Rect, outer: Rect) -> Rect {
    let x = inner.x.max(outer.x);
    let y = inner.y.max(outer.y);
    let right = inner.x.saturating_add(inner.width).min(outer.right());
    let bottom = inner.y.saturating_add(inner.height).min(outer.bottom());
    Rect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
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
