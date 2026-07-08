//! Reusable small widgets.

use crate::ui::theme;
use ratatui::text::{Line, Span};

pub fn spinner_frame(tick: u64) -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

/// A `(shortcut, description)` pair rendered as `key label`.
pub type KeyHint = (&'static str, &'static str);

const HINT_GAP: &str = "   ";

/// Render key hints as one styled line: accent keys, dim labels.
pub fn key_hint_line(pairs: &[KeyHint]) -> Line<'static> {
    let mut spans = Vec::with_capacity(pairs.len() * 3);
    for (i, (key, label)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(HINT_GAP));
        }
        spans.push(Span::styled(*key, theme::key_style()));
        if !label.is_empty() {
            spans.push(Span::styled(format!(" {label}"), theme::dim()));
        }
    }
    Line::from(spans)
}

/// Display width of a hint line, for width-aware layout decisions.
pub fn key_hint_width(pairs: &[KeyHint]) -> usize {
    let mut w = 0;
    for (i, (key, label)) in pairs.iter().enumerate() {
        if i > 0 {
            w += HINT_GAP.len();
        }
        w += key.chars().count();
        if !label.is_empty() {
            w += 1 + label.chars().count();
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_width_matches_rendered_content() {
        let pairs: &[KeyHint] = &[("Space", "select"), ("d", "clean")];
        let line = key_hint_line(pairs);
        let rendered: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(key_hint_width(pairs), rendered);
    }
}
