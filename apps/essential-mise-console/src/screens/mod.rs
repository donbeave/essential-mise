//! TUI screens (P1, gated on G1).
//!
//! Shared modules come first (Termrock rule): `filter` (S1+S2),
//! `rerun_fmt` (S3+S4), `chrome` (all screens). Per-screen modules land
//! in T4.2–T4.5 and must reuse these — no per-screen copies.

/// S3 Apply: live two-phase apply with progress, cancel, and rerun.
pub mod apply;
/// PageHeader/StatusBar/HintBar wiring (all screens).
pub mod chrome;
/// Live-substring filter with hidden-rows-keep-state (S1+S2).
pub mod filter;
/// Step 1 Modules: multi-select with search.
pub mod modules;
/// S2 Packages: per-package checklist grouped by declaring module.
pub mod packages;
/// Plain-text painting: glyph sets, rows, footer, primary line.
pub mod paint;
/// Exact rerun-command formatting (S3+S4).
pub mod rerun_fmt;
/// S4 Result: verify screen with drift table and Done gate.
pub mod result;
