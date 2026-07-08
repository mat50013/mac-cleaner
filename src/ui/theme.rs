//! Color palette and style helpers.
//!
//! Terminal.app does not render truecolor reliably, so the palette falls back
//! to xterm-256 colors when needed.

use crate::model::Category;
use ratatui::style::{Color, Modifier, Style};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy)]
struct Palette {
    surface: Color,
    border: Color,
    text: Color,
    dim: Color,
    accent: Color,
    safe: Color,
    moderate: Color,
    risky: Color,
    highlight: Color,
    selection_bg: Color,
    cat_caches: Color,
    cat_logs: Color,
    cat_dev_artifacts: Color,
    cat_duplicates: Color,
    cat_icloud: Color,
    cat_large: Color,
    cat_trash: Color,
    modal_backdrop: Color,
    modal_shadow: Color,
    modal_elevated: Color,
}

static PALETTE: OnceLock<Palette> = OnceLock::new();

pub fn init() {
    let _ = PALETTE.set(Palette::for_terminal());
}

fn palette() -> &'static Palette {
    PALETTE.get_or_init(Palette::for_terminal)
}

/// Whether the current terminal is expected to render `Color::Rgb` correctly.
pub fn supports_truecolor() -> bool {
    if terminal_is_apple_terminal() {
        return false;
    }
    std::env::var("COLORTERM")
        .map(|v| {
            let v = v.to_lowercase();
            v.contains("truecolor") || v.contains("24bit")
        })
        .unwrap_or(false)
        || std::env::var("TERM_PROGRAM")
            .map(|p| {
                matches!(
                    p.as_str(),
                    "iTerm.app" | "WezTerm" | "vscode" | "Hyper" | "kitty" | "ghostty"
                )
            })
            .unwrap_or(false)
}

fn terminal_is_apple_terminal() -> bool {
    std::env::var("TERM_PROGRAM")
        .map(|t| t == "Apple_Terminal")
        .unwrap_or(false)
}

impl Palette {
    fn for_terminal() -> Self {
        if supports_truecolor() {
            Self::truecolor()
        } else {
            Self::indexed()
        }
    }

    fn truecolor() -> Self {
        Self {
            surface: Color::Rgb(28, 28, 38),
            border: Color::Rgb(100, 100, 125),
            text: Color::Rgb(220, 220, 230),
            dim: Color::Rgb(175, 175, 200),
            accent: Color::Rgb(120, 180, 255),
            safe: Color::Rgb(100, 220, 140),
            moderate: Color::Rgb(240, 200, 80),
            risky: Color::Rgb(240, 100, 100),
            highlight: Color::Rgb(180, 140, 255),
            selection_bg: Color::Rgb(48, 54, 80),
            cat_caches: Color::Rgb(100, 180, 255),
            cat_logs: Color::Rgb(255, 175, 90),
            cat_dev_artifacts: Color::Rgb(120, 220, 170),
            cat_duplicates: Color::Rgb(180, 140, 255),
            cat_icloud: Color::Rgb(90, 210, 230),
            cat_large: Color::Rgb(255, 120, 175),
            cat_trash: Color::Rgb(220, 110, 110),
            modal_backdrop: Color::Rgb(10, 10, 16),
            modal_shadow: Color::Rgb(4, 4, 8),
            modal_elevated: Color::Rgb(44, 44, 58),
        }
    }

    fn indexed() -> Self {
        Self {
            surface: Color::Indexed(235),
            border: Color::Indexed(250),
            text: Color::Indexed(252),
            dim: Color::Indexed(251),
            accent: Color::Indexed(111),
            safe: Color::Indexed(78),
            moderate: Color::Indexed(221),
            risky: Color::Indexed(203),
            highlight: Color::Indexed(141),
            selection_bg: Color::Indexed(238),
            cat_caches: Color::Indexed(111),
            cat_logs: Color::Indexed(215),
            cat_dev_artifacts: Color::Indexed(115),
            cat_duplicates: Color::Indexed(141),
            cat_icloud: Color::Indexed(87),
            cat_large: Color::Indexed(213),
            cat_trash: Color::Indexed(203),
            modal_backdrop: Color::Indexed(233),
            modal_shadow: Color::Indexed(232),
            modal_elevated: Color::Indexed(237),
        }
    }
}

pub fn surface() -> Color {
    palette().surface
}

pub fn accent() -> Color {
    palette().accent
}

pub fn safe() -> Color {
    palette().safe
}

pub fn highlight() -> Color {
    palette().highlight
}

pub fn selection_bg() -> Color {
    palette().selection_bg
}

pub fn title_style() -> Style {
    Style::default()
        .fg(palette().accent)
        .add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(palette().dim)
}

/// Style for keyboard shortcut "caps" in hints and legends.
pub fn key_style() -> Style {
    Style::default()
        .fg(palette().accent)
        .add_modifier(Modifier::BOLD)
}

/// Style for the cursor row in tables and lists.
pub fn selected_row_style() -> Style {
    Style::default()
        .bg(palette().selection_bg)
        .fg(palette().text)
        .add_modifier(Modifier::BOLD)
}

pub fn text() -> Style {
    Style::default().fg(palette().text)
}

pub fn tier_style(tier: crate::model::SafetyTier) -> Style {
    let p = palette();
    match tier {
        crate::model::SafetyTier::Safe => Style::default().fg(p.safe),
        crate::model::SafetyTier::Moderate => Style::default().fg(p.moderate),
        crate::model::SafetyTier::Risky => Style::default().fg(p.risky),
    }
}

pub fn category_color(cat: Category) -> Color {
    let p = palette();
    match cat {
        Category::Caches => p.cat_caches,
        Category::Logs => p.cat_logs,
        Category::DevArtifacts => p.cat_dev_artifacts,
        Category::Duplicates => p.cat_duplicates,
        Category::ICloud => p.cat_icloud,
        Category::LargeFiles => p.cat_large,
        Category::Trash => p.cat_trash,
    }
}

pub fn block(title: &str) -> ratatui::widgets::Block<'_> {
    let p = palette();
    ratatui::widgets::Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title)
        .title_style(title_style())
        .style(Style::default().fg(p.border).bg(p.surface))
}

pub fn modal_backdrop() -> Style {
    Style::default().bg(palette().modal_backdrop)
}

pub fn modal_shadow() -> Style {
    Style::default().bg(palette().modal_shadow)
}

pub fn modal_block(title: &str) -> ratatui::widgets::Block<'_> {
    let p = palette();
    ratatui::widgets::Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title)
        .title_style(title_style())
        .style(Style::default().fg(p.accent).bg(p.modal_elevated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_palette_uses_no_rgb() {
        let p = Palette::indexed();
        assert!(matches!(p.accent, Color::Indexed(_)));
        assert!(matches!(p.safe, Color::Indexed(_)));
    }

    #[test]
    fn truecolor_palette_uses_rgb() {
        let p = Palette::truecolor();
        assert!(matches!(p.accent, Color::Rgb(_, _, _)));
    }
}
