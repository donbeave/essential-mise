//! Plain-text painting shared by the Review, Result and help screens.
//!
//! Every state is a glyph plus a word, never colour alone. When the theme
//! uses the ASCII glyph set (`TERM=dumb`, `JUNIE_TUI_GLYPHS=ascii`) the
//! glyphs fall back to `[ok]`, `[!]`, ...; `NO_COLOR` selects the mono colour
//! level in the theme itself.

use std::borrow::Cow;

use junie_tui::theme::{Glyphs, Theme, Tone};
use junie_tui::widgets::hintbar::{HintBar, HintLayer};
use junie_tui::widgets::keyhint::{Hint, HintPriority};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Glyphs for states and chrome, Unicode or ASCII.
#[derive(Debug, Clone, Copy)]
pub struct Marks {
    /// Up to date.
    pub ok: &'static str,
    /// Needs changes.
    pub warn: &'static str,
    /// Failed.
    pub fail: &'static str,
    /// Skipped or not checked.
    pub skip: &'static str,
    /// Could not check.
    pub unknown: &'static str,
    /// A plain list bullet.
    pub bullet: &'static str,
    /// Focus marker.
    pub focus: &'static str,
    /// The primary-action arrow.
    pub primary: &'static str,
    /// Collapsed section.
    pub closed: &'static str,
    /// Expanded section.
    pub open: &'static str,
    /// Truncation mark.
    pub ellipsis: &'static str,
    /// Separator between footer items and header parts.
    pub dot: &'static str,
    /// Busy frames.
    pub spinner: &'static [&'static str],
}

impl Marks {
    /// The Unicode set.
    pub const fn unicode() -> Self {
        Self {
            ok: "✓",
            warn: "!",
            fail: "✗",
            skip: "–",
            unknown: "?",
            bullet: "·",
            focus: "▸",
            primary: "▶",
            closed: "▸",
            open: "▾",
            ellipsis: "…",
            dot: "·",
            spinner: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
        }
    }

    /// Pick the set that matches the theme's glyph table.
    pub fn for_theme(theme: &Theme) -> Self {
        if theme.glyphs == Glyphs::ASCII {
            Self {
                ok: "[ok]",
                warn: "[!]",
                fail: "[x]",
                skip: "[-]",
                unknown: "[?]",
                bullet: "-",
                focus: ">",
                primary: ">",
                closed: "+",
                open: "-",
                ellipsis: "...",
                dot: "-",
                spinner: &["|", "/", "-", "\\"],
            }
        } else {
            Self::unicode()
        }
    }

    /// Spinner frame for an animation tick.
    pub fn frame(&self, tick: u64) -> &'static str {
        self.spinner[(tick as usize) % self.spinner.len()]
    }
}

/// A styled run of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seg {
    /// The text.
    pub text: String,
    /// Its tone.
    pub tone: Tone,
    /// Bold.
    pub bold: bool,
}

/// One painted line.
pub type Row = Vec<Seg>;

/// A plain segment.
pub fn seg(text: impl Into<String>, tone: Tone) -> Seg {
    Seg {
        text: text.into(),
        tone,
        bold: false,
    }
}

/// A bold segment.
pub fn bold(text: impl Into<String>, tone: Tone) -> Seg {
    Seg {
        text: text.into(),
        tone,
        bold: true,
    }
}

/// Plain text of a row (tests, copying).
pub fn row_text(row: &[Seg]) -> String {
    row.iter().map(|s| s.text.as_str()).collect()
}

/// Truncate `text` to `width` cells with the theme's ellipsis.
pub fn fit(text: &str, width: usize, ellipsis: &str) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let keep = width.saturating_sub(ellipsis.chars().count());
    let mut out: String = text.chars().take(keep).collect();
    out.push_str(ellipsis);
    out.chars().take(width).collect()
}

/// Wrap `text` to `width` cells on word boundaries (long words are split).
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        while word.chars().count() > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            lines.push(word.chars().take(width).collect());
            word = word.chars().skip(width).collect();
        }
        let extra = usize::from(!current.is_empty());
        if current.chars().count() + extra + word.chars().count() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(&word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Paint `row` at `(x, y)` within `width` cells, truncating with an ellipsis.
pub fn put(buf: &mut Buffer, x: u16, y: u16, width: u16, row: &[Seg], theme: &Theme, bg: Color) {
    let marks = Marks::for_theme(theme);
    let mut remaining = width as usize;
    let mut cx = x;
    let total: usize = row.iter().map(|s| s.text.chars().count()).sum();
    let truncate = total > remaining;
    for (i, part) in row.iter().enumerate() {
        if remaining == 0 {
            break;
        }
        let last = i + 1 == row.len();
        let mut text = part.text.clone();
        let len = text.chars().count();
        if len > remaining || (truncate && last) {
            text = fit(&text, remaining, marks.ellipsis);
        }
        let style = Style::new()
            .fg(theme.tone_on(part.tone, bg))
            .bg(bg)
            .add_modifier(if part.bold {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let used = text.chars().count();
        buf.set_stringn(cx, y, &text, used, style);
        cx += used as u16;
        remaining -= used.min(remaining);
    }
}

/// Fill `area` with the canvas colour.
pub fn clear(buf: &mut Buffer, area: Rect, bg: Color) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            buf[(x, y)].set_symbol(" ").set_style(Style::new().bg(bg));
        }
    }
}

/// Footer hints generated from the keys a screen has enabled right now.
pub fn footer(area: Rect, buf: &mut Buffer, theme: &Theme, hints: &[(&'static str, String)]) {
    let layer = HintLayer::new(
        hints
            .iter()
            .map(|(key, action)| Hint {
                key,
                action: Cow::Owned(action.clone()),
                priority: HintPriority::Auto,
            })
            .collect(),
    );
    HintBar::render_at(area, buf, theme, &layer);
}

/// The one primary-action line: `▶ Enter  <label>`.
pub fn primary_line(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    bg: Color,
    key: &str,
    label: &str,
) {
    let marks = Marks::for_theme(theme);
    let row = vec![
        seg(format!(" {} ", marks.primary), Tone::Active),
        bold(key.to_string(), Tone::Active),
        seg("  ", Tone::Normal),
        bold(label.to_string(), Tone::Normal),
    ];
    put(buf, area.x, area.y, area.width, &row, theme, bg);
}

/// A bordered box with a title; returns the inner area.
pub fn framed(area: Rect, buf: &mut Buffer, theme: &Theme, bg: Color, title: &str) -> Rect {
    use ratatui::symbols::border;
    let set = if theme.glyphs == Glyphs::ASCII {
        border::Set {
            top_left: "+",
            top_right: "+",
            bottom_left: "+",
            bottom_right: "+",
            vertical_left: "|",
            vertical_right: "|",
            horizontal_top: "-",
            horizontal_bottom: "-",
        }
    } else {
        border::PLAIN
    };
    if area.width < 4 || area.height < 2 {
        return area;
    }
    let style = Style::new().fg(theme.border_strong).bg(bg);
    let right = area.right() - 1;
    let bottom = area.bottom() - 1;
    for x in area.x + 1..right {
        buf[(x, area.y)]
            .set_symbol(set.horizontal_top)
            .set_style(style);
        buf[(x, bottom)]
            .set_symbol(set.horizontal_bottom)
            .set_style(style);
    }
    for y in area.y + 1..bottom {
        buf[(area.x, y)]
            .set_symbol(set.vertical_left)
            .set_style(style);
        buf[(right, y)]
            .set_symbol(set.vertical_right)
            .set_style(style);
        for x in area.x + 1..right {
            buf[(x, y)].set_symbol(" ").set_style(Style::new().bg(bg));
        }
    }
    buf[(area.x, area.y)]
        .set_symbol(set.top_left)
        .set_style(style);
    buf[(right, area.y)]
        .set_symbol(set.top_right)
        .set_style(style);
    buf[(area.x, bottom)]
        .set_symbol(set.bottom_left)
        .set_style(style);
    buf[(right, bottom)]
        .set_symbol(set.bottom_right)
        .set_style(style);
    if !title.is_empty() {
        let label = format!(" {title} ");
        let room = (area.width as usize).saturating_sub(4);
        let label = fit(&label, room, "");
        buf.set_stringn(
            area.x + 2,
            area.y,
            &label,
            room,
            Style::new().fg(theme.text_secondary).bg(bg),
        );
    }
    Rect::new(area.x + 2, area.y + 1, area.width - 4, area.height - 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_truncates_with_the_given_ellipsis() {
        assert_eq!(fit("abcdef", 4, "…"), "abc…");
        assert_eq!(fit("abc", 4, "…"), "abc");
        assert_eq!(fit("abcdef", 5, "..."), "ab...");
    }

    #[test]
    fn wrap_breaks_on_words_and_splits_long_words() {
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap("", 4), vec![String::new()]);
    }
}
