//! The `?` overlay: the keys that work on the current screen, plus the few
//! words the interface uses. The vocabulary is defined once, here.

use termrock::core::id::WidgetId;
use termrock::widgets::help::{HelpOverlay, HelpSection};

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

/// Build the keyboard help from the active screen's available actions.
pub fn overlay(screen_keys: &[(&'static str, String)]) -> HelpOverlay {
    let screen = screen_keys.iter().fold(
        HelpSection::new("Keys on this screen"),
        |section, (key, action)| section.row(*key, action),
    );
    let common = COMMON_KEYS
        .iter()
        .fold(HelpSection::new("Everywhere"), |section, (key, action)| {
            section.row(*key, *action)
        });
    let vocabulary = VOCABULARY.iter().fold(
        HelpSection::new("Words used here"),
        |mut section, (word, meaning)| {
            section.rows.push(((*word).into(), (*meaning).into()));
            section
        },
    );
    HelpOverlay::from_keymaps(
        WidgetId::of("console-help"),
        "essential-mise",
        vec![screen, common, vocabulary],
    )
}
