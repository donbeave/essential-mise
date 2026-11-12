#![warn(missing_docs)]
//! essential_mise_console library.

/// Console app: routing, selection memory, and the apply event pump.
pub mod app;
/// Stock-`mise bootstrap` backend: read-only queries plus two-phase apply.
pub mod backend;
/// Clap command tree (spec §2 verbatim) plus exit-code contract.
pub mod cli;
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
