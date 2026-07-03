//! Color palette and style helpers.

use ratatui::style::{Color, Modifier, Style};

pub const SURFACE: Color = Color::Rgb(28, 28, 38);
pub const BORDER: Color = Color::Rgb(60, 60, 80);
pub const TEXT: Color = Color::Rgb(220, 220, 230);
pub const DIM: Color = Color::Rgb(130, 130, 150);
pub const ACCENT: Color = Color::Rgb(120, 180, 255);
pub const SAFE: Color = Color::Rgb(100, 220, 140);
pub const MODERATE: Color = Color::Rgb(240, 200, 80);
pub const RISKY: Color = Color::Rgb(240, 100, 100);
pub const HIGHLIGHT: Color = Color::Rgb(180, 140, 255);

pub fn title_style() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(DIM)
}

pub fn text() -> Style {
    Style::default().fg(TEXT)
}

pub fn tier_style(tier: crate::model::SafetyTier) -> Style {
    match tier {
        crate::model::SafetyTier::Safe => Style::default().fg(SAFE),
        crate::model::SafetyTier::Moderate => Style::default().fg(MODERATE),
        crate::model::SafetyTier::Risky => Style::default().fg(RISKY),
    }
}

pub fn block(title: &str) -> ratatui::widgets::Block<'_> {
    ratatui::widgets::Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title)
        .style(Style::default().fg(BORDER).bg(SURFACE))
}
