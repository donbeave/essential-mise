//! The `?` overlay: the keys that work on the current screen, plus the few
//! words the interface uses. The vocabulary is defined once, here.

use junie_tui::theme::{Theme, Tone};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::paint::{self, Marks, bold, framed, put, seg};

/// What the words mean. One table; screens never define their own.
pub const VOCABULARY: &[(&str, &str)] = &[
    ("module", "a bundle of apps and settings you can switch on"),
    (
        "area",
        "one kind of thing a module manages: apps, command-line tools, macOS settings, login items, dotfiles, login shell",
    ),
    ("Up to date", "already matches what you asked for"),
    ("Needs changes", "differs, and applying will change it"),
    ("Failed", "could not be applied"),
    ("Skipped", "left out on purpose"),
    (
        "Install / Skip this time",
        "your choice for this run only; it is not saved",
    ),
];

/// Keys that work on every screen.
pub const COMMON_KEYS: &[(&str, &str)] = &[
    ("Enter", "do the main action shown above the footer"),
    ("Esc", "go back or close"),
    ("q", "quit"),
    ("?", "show or hide this help"),
];

/// Paint the overlay over `area`: `screen_keys` first, then common keys and
/// the vocabulary. Anything that does not fit is cut at the bottom, with the
/// title telling the person how to close it.
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    bg: Color,
    screen_keys: &[(&'static str, String)],
) {
    let marks = Marks::for_theme(theme);
    let width = area.width.saturating_sub(4).min(92);
    let mut lines: Vec<Vec<paint::Seg>> = Vec::new();
    lines.push(vec![bold("Keys on this screen", Tone::Normal)]);
    for (key, action) in screen_keys {
        lines.push(vec![
            seg(format!("  {key:<8}"), Tone::Active),
            seg(action.clone(), Tone::Normal),
        ]);
    }
    lines.push(Vec::new());
    lines.push(vec![bold("Everywhere", Tone::Normal)]);
    for (key, action) in COMMON_KEYS {
        lines.push(vec![
            seg(format!("  {key:<8}"), Tone::Active),
            seg((*action).to_string(), Tone::Normal),
        ]);
    }
    lines.push(Vec::new());
    lines.push(vec![bold("Words used here", Tone::Normal)]);
    for (word, meaning) in VOCABULARY {
        let text = format!("{word}: {meaning}");
        for (i, part) in paint::wrap(&text, width.saturating_sub(6) as usize)
            .into_iter()
            .enumerate()
        {
            let lead = if i == 0 { "  " } else { "    " };
            lines.push(vec![seg(format!("{lead}{part}"), Tone::Secondary)]);
        }
    }
    let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let inner = framed(
        Rect::new(x, y, width, height),
        buf,
        theme,
        bg,
        &format!("Help {} Esc or ? closes", marks.dot),
    );
    for (i, row) in lines.iter().take(inner.height as usize).enumerate() {
        put(
            buf,
            inner.x,
            inner.y + i as u16,
            inner.width,
            row,
            theme,
            bg,
        );
    }
}
