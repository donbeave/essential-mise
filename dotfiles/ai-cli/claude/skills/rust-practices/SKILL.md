---
name: rust-practices
description: Rust practices for implementing, reviewing or profiling code in a Cargo workspace, including clap CLIs and ratatui/crossterm TUIs. Use when working in a repo with Cargo.toml.
---

# Rust practices

The repo's own lint posture, toolchain and wrapper win over this file.

## Before editing
Read `[workspace.lints]`, `clippy.toml`, `rust-toolchain.toml`, any cargo wrapper (`.cargo/config.toml`, `mise.toml`), and the code you change with its callers.

## Code
- **API design** (Rust API Guidelines): newtypes over bare `bool`, `Option` or primitive arguments; private fields; sealed traits where extension is not intended; `Debug` on public types; documented `# Errors` and `# Panics`.
- **Errors**: `thiserror` enums in libraries with `#[from]` and `#[source]`; `anyhow` with `.context()` only in applications. No `unwrap`, `expect` or `panic` on reachable production paths.
- **Edition 2024**: `unsafe_op_in_unsafe_fn`, `env::set_var` is unsafe, `unsafe extern`, earlier drop of tail-expression and `if let` temporaries, resolver 3.
- **Async (tokio)**: never hold a `std` mutex guard across `.await`; know cancel safety in `select!` (`recv` is safe; `read_exact`, `write_all` and lock waits are not); bounded blocking work in `spawn_blocking`, long-lived work on its own thread; shutdown via `CancellationToken` and `TaskTracker`; bounded channels.
- **Lints**: `#[expect(lint, reason = "...")]` over `#[allow]`, so stale suppressions are flagged.
- **Dependencies**: inherit from `[workspace.dependencies]`.
- **Tests**: `cargo nextest` where configured; it skips doctests, so run `cargo test --doc` when the repo has doctests and allows it. `proptest` for parsers and round-trips.

## CLIs (clap derive)
`Parser`, `Subcommand`, `Args` with `#[command(flatten)]`, `ValueEnum`. Map errors to exit codes in `main`. Add a `Command::debug_assert()` test if the repo has none. If the repo denies print lints, write through its output layer.

## TUIs (ratatui, crossterm)
`ratatui::init` or `try_init` installs a terminal-restoring panic hook: call it after any other panic hooks, and return draw errors so the restore runs. Measure text with `unicode-width`. PTY resize is forwarded as SIGWINCH. Some repos enforce pure rendering with lints; honor them.

## Verify
Through the repo's wrapper: `cargo check -p <crate>`, `cargo clippy -p <crate> --all-targets -- -D warnings`, the crate's tests, then the fast gate. For CLIs, `cargo run -p <bin> -- <args>`.

## Profiling and audit
Release builds with frame pointers and line tables; `samply`, `cargo flamegraph` or Instruments; DHAT for allocations; `criterion` for microbenchmarks; `cargo build --timings` and `cargo-llvm-lines` for compile time. `cargo deny check` and `cargo audit` for advisories and licenses.
