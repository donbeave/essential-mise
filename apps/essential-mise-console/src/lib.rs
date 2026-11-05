#![warn(missing_docs)]
//! essential_mise_console library.

/// Stock-`mise bootstrap` backend: read-only queries plus two-phase apply.
pub mod backend;
/// Clap command tree (spec §2 verbatim) plus exit-code contract.
pub mod cli;
/// TUI screens (G1-gated); shared modules first, per-screen in T4.2–T4.5.
pub mod screens;
