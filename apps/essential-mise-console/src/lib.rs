#![warn(missing_docs)]
//! essential_mise_console library.

/// Console app: routing, selection memory, and the apply event pump.
pub mod app;
/// Stock-`mise bootstrap` backend: read-only queries plus two-phase apply.
pub mod backend;
/// Status checks off the UI thread.
pub mod check;
/// Clap command tree (spec §2 verbatim) plus exit-code contract.
pub mod cli;
/// Typed follow-ups after an apply: relaunch hints, skipped hooks, new shell.
pub mod followup;
/// Config repository lint (`lint` subcommand).
pub mod lint;
/// `#:` header comments in config files: the engine/config contract.
pub mod metadata;
/// TUI screens (G1-gated); shared modules first, per-screen in T4.2–T4.5.
pub mod screens;
/// `setup` stages 1-6: staged plan data and executor.
pub mod setup;
/// Config repository source (`setup --config`): normalization and matching.
pub mod source;
/// Typed `mise bootstrap status --json`: areas, entries, verdicts.
pub mod status;
