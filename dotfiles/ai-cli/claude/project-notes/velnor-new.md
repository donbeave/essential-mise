# velnor-new (tailrocks/velnor-new)

## What it is
Velnor generates GitHub Actions workflows for any stack (`velnor-actions init|plan|generate`) and runs self-hosted runners via GitHub's Runner Scale Set protocol. Root workspace: `velnor-actions-contract` (shared types), stack adapters (`-rust`, `-bun`, `-tofu`, `-gradle`, `-dbt`, `-ansible`, `-mise`, `-actionlint`), `-orchestrator` (core), `-workflow-renderer`, `-cli`, `-freshness`, `velnor-archive-guard`. Nested workspace `crates/velnor-runner` (own lockfile and deny.toml): core, github (Scale Set API), host (macOS supervisor, Docker via bollard, turso journal, launchd), cli, attestation (sigstore/TUF).

## Stack
Rust 1.99.x, edition 2024; generator is synchronous (serde, toml, thiserror, clap); runner is tokio + reqwest + bollard + turso. No unsafe or FFI.

## Gates
- `bash scripts/verify-local.sh`; `alint validate-config && alint check --fail-on-warning`; `cargo deny check` plus `--manifest-path crates/velnor-runner/Cargo.toml`.
- Root `--workspace` does not cover `crates/velnor-runner`: run fmt and clippy there separately.
- Narrow: `cargo nextest run --locked -p <pkg> [-E 'test(<f>)'] --no-tests fail`. Cargo is wrapped by mbx via mise.
- Many test targets are `test = false` during the integration-first rebuild; restoring a test may require re-enabling its Cargo target.

## Rules that change how agents work
- `unsafe_code` forbid; clippy `all` + `pedantic`; denied `unwrap_used`, `expect_used`, `panic`, `todo`, `dbg_macro`, `too_many_lines` (80), `await_holding_lock`, `allow_attributes_without_reason`. Exact `=x.y.z` pins.
- No code comments. nextest only. Test plans and red/green proof need an independent subagent.
- Read `docs/hard-decisions.md` first; changing a hard decision needs owner approval.
- Never guess GitHub protocol behavior: check actions/runner, actions/scaleset, actions/runner-images source; record in `docs/proposed/github-actions-reference.md`.
- Read the cache docs before touching cache keys.

## Ownership and generated files
Velnor generates its own `.github/workflows/`. Consumer repos (jackin, ChainArgos) get workflows from a released, digest-pinned `velnor-actions`; generator bugs are fixed here, released, then regenerated in consumers.

## Where decisions live
`docs/hard-decisions.md`, `docs/implemented/`, `docs/proposed/`, `qualification/` (hosted vs Scale Set comparison fixtures), `.cargo/mutants.toml` (riskiest areas: parsers, identities, selection, aggregation).
