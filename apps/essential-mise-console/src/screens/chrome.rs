//! Screen chrome wiring shared by S1–S4 (Termrock rule).
//!
//! Every screen maps ONLY to inventoried `junie-tui` components at the pinned
//! rev: [`PageHeader`](junie_tui::widgets::page_header::PageHeader),
//! [`StatusBar`](junie_tui::widgets::statusbar::StatusBar) with
//! [`StatusItem`](junie_tui::widgets::statusbar::StatusItem), and the
//! [`HintLayer`](junie_tui::widgets::hintbar::HintLayer) built from
//! [`hint`](junie_tui::widgets::keyhint::hint) rows. Screens must not build
//! these widgets any other way.

use junie_tui::theme::Tone;
use junie_tui::widgets::hintbar::HintLayer;
use junie_tui::widgets::keyhint::hint;
use junie_tui::widgets::page_header::PageHeader;
use junie_tui::widgets::statusbar::{StatusBar, StatusItem};

/// Console title shown on every screen.
pub const TITLE: &str = "essential-mise-console";

/// Header: title plus the `N/4 Screen` step indicator on the right.
pub fn page_header(step: u8, screen: &str) -> PageHeader {
    PageHeader::new(TITLE).right(&format!("{step}/4 {screen}"))
}

/// Status bar from explicit left/right items (screens own the content,
/// this owns the construction).
pub fn status_bar(left: Vec<StatusItem>, right: Vec<StatusItem>) -> StatusBar {
    let mut bar = StatusBar::new();
    bar.left = left;
    bar.right = right;
    bar
}

/// One status item with default priority.
pub fn status_item(text: impl Into<String>, tone: Tone) -> StatusItem {
    StatusItem::new(text, tone)
}

/// Hint layer from static `(key, action)` pairs.
pub fn hint_layer(hints: &[(&'static str, &'static str)]) -> HintLayer {
    HintLayer::new(hints.iter().map(|(k, a)| hint(k, a)).collect())
}

use junie_tui::theme::Theme;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::widgets::hintbar::HintBar;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Paint the shared page header.
pub fn render_header(step: u8, screen: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
    page_header(step, screen).render(area, buf, theme);
}

/// Paint the shared static hint bar.
pub fn render_hints(
    hints: &[(&'static str, &'static str)],
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    HintBar::render_at(area, buf, theme, &hint_layer(hints));
}

/// Paint the shared status bar with one muted left item.
pub fn render_status(left: &str, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
    status_bar(vec![status_item(left, Tone::Muted)], Vec::new()).render(area, buf, ctx);
}
