//! TUI screens (P1, gated on G1).
//!
//! Shared modules come first (Termrock rule): [`filter`] (S1+S2),
//! [`rerun_fmt`] (S3+S4), [`chrome`] (all screens). Per-screen modules land
//! in T4.2–T4.5 and must reuse these — no per-screen copies.

/// PageHeader/StatusBar/HintBar wiring (all screens).
pub mod chrome;
/// Live-substring filter with hidden-rows-keep-state (S1+S2).
pub mod filter;
/// Exact rerun-command formatting (S3+S4).
pub mod rerun_fmt;
