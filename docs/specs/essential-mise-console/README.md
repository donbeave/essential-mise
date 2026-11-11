# essential-mise-console - spec index

Rust CLI and TUI frontend over the stock `mise bootstrap` engine, built on the
Termrock component library (`junie-tui`). The module catalogue comes from the
`essential-mise.toml` manifest of a config repository (see
`docs/config-repo.md`); the console never hardcodes module names.

| Doc | Contents |
|---|---|
| `spec.md` | Goal, non-goals, architecture, screens, phases, verification |
| `implementation-spec.md` | Build-ready CLI and TUI spec: commands, screens, backend trait, scenarios |

## Decisions

- The engine stays `mise bootstrap`; TOML modules stay the source of truth for packages.
- The module list and descriptions are declared in `essential-mise.toml`
  (`[[modules]]` tables), not in shell arrays.
- The crate lives in this repository at `apps/essential-mise-console/` and
  depends on `junie-tui` by pinned git revision, never a branch.
- Distribution: prebuilt release binaries; `bootstrap.sh` is a small fetcher
  that verifies a pinned checksum and execs the binary (`setup`).

## Open questions

1. Unapply scope: recommended for a later phase.
2. Visual identity: functional first, review later.
3. Persistent selection editing: read-only hint first, editor later.
