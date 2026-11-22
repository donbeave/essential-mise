//! Plain-text painting shared by the Review, Result and help screens.
//!
//! Every state is a glyph plus a word, never colour alone. When the theme
//! uses the ASCII glyph set (`TERM=dumb`, `TERMROCK_GLYPHS=ascii`) the
//! glyphs fall back to `[ok]`, `[!]`, ...; `NO_COLOR` selects the mono colour
//! level in the theme itself.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use termrock::theme::{Glyphs, Theme, Tone};
use termrock::widgets::hintbar::{HintBar, HintLayer};
use termrock::widgets::keyhint::{Hint, HintPriority};

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

/// A styled run of text supplied by Termrock.
pub use termrock::widgets::viewport::Span as Seg;

/// One painted line.
pub type Row = termrock::widgets::viewport::Line;

/// A plain segment.
pub fn seg(text: impl Into<String>, tone: Tone) -> Seg {
    Seg::new(text, tone)
}

/// A bold segment.
pub fn bold(text: impl Into<String>, tone: Tone) -> Seg {
    Seg::new(text, tone).bold()
}

/// Plain text of a row (tests, copying).
pub fn row_text(row: &[Seg]) -> String {
    row.iter().map(|s| s.text.as_str()).collect()
}

/// Truncate `text` to `width` cells with the theme's ellipsis.
pub fn fit(text: &str, width: usize, ellipsis: &str) -> String {
    use termrock::ui::text;
    if text::width(text) <= width {
        return text.to_owned();
    }
    let suffix = text::clip(ellipsis, width);
    format!(
        "{}{}",
        text::clip(text, width.saturating_sub(text::width(&suffix))),
        suffix
    )
}

/// Wrap text on word boundaries using terminal cell widths.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let lines = termrock::ui::text::wrap(text, width.max(1));
    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

/// Paint a Termrock text row within the available width.
pub fn put(buf: &mut Buffer, x: u16, y: u16, width: u16, row: &[Seg], theme: &Theme, bg: Color) {
    termrock::widgets::viewport::render_spans(buf, x, y, width, theme, row, bg);
}

/// Fill an area with its background colour.
pub fn clear(buf: &mut Buffer, area: Rect, bg: Color) {
    termrock::ui::ctx::fill(buf, area, Style::new().bg(bg));
}

/// Footer hints generated from the keys a screen has enabled right now.
pub fn footer(area: Rect, buf: &mut Buffer, theme: &Theme, hints: &[(&'static str, String)]) {
    let layer = HintLayer::new(
        hints
            .iter()
            .map(|(key, action)| Hint {
                key: (*key).into(),
                action: Cow::Owned(action.clone()),
                priority: HintPriority::Auto,
                click_key: None,
            })
            .collect(),
    );
    HintBar::render_at(area, buf, theme, &layer);
}

/// The current screen's primary action.
pub const PRIMARY: termrock::core::id::WidgetId =
    termrock::core::id::WidgetId::of("console-primary");

/// Render the screen's primary action through the shared button component.
pub fn primary_line(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut termrock::ui::ctx::RenderCtx,
    bg: Color,
    key: &str,
    label: &str,
) {
    let label = if key.is_empty() {
        label.to_owned()
    } else {
        format!("{key}  {label}")
    };
    termrock::widgets::button::Button::primary(PRIMARY, &label)
        .disabled(key.is_empty())
        .focusable(false)
        .render(area, buf, ctx, bg);
}

/// A bordered box with a title; returns the inner area.
pub fn framed(area: Rect, buf: &mut Buffer, theme: &Theme, bg: Color, title: &str) -> Rect {
    let mut panel = termrock::widgets::panel::Panel::framed(Some(title)).pad_x(1);
    panel.bg_override = Some(bg);
    panel.render(area, buf, theme)
}
