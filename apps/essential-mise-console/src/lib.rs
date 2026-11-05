#![warn(missing_docs)]
//! essential_mise_console library.

/// Stock-`mise bootstrap` backend: read-only queries plus two-phase apply.
pub mod backend;
/// Clap command tree (spec §2 verbatim) plus exit-code contract.
pub mod cli;
