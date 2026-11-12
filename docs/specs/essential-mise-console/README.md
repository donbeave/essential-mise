# essential-mise-console - spec index

Rust CLI and TUI frontend over the stock `mise bootstrap` engine, built on the
Termrock component library (`junie-tui`). The module catalogue comes from the
`#:` header comments of a config repository's files (see
`docs/config-repo.md`); the console never hardcodes module names.

**Status.** These files are the design record from before v0.2.0. Where they
differ from the shipped console, `README.md`, `docs/config-repo.md`, and
`essential-mise-console --help` win. Known differences in v0.2.0:

| Spec says | v0.2.0 |
| --- | --- |
| `setup` runs stages 1-5 | `setup` runs six stages: Command Line Tools, Homebrew, mise, repository access, adoption of the config repository, base modules |
| `setup` takes no arguments | `setup [--config SOURCE] [--dry-run]`; `--config` falls back to `$ESSENTIAL_MISE_CONFIG` |
| Module catalogue from the `config.<name>.toml` files, 29 environments | Module catalogue from the `#:` headers of your own config repository; the engine ships none |
| `status` and `verify` commands | Listed in `--help`, not implemented yet: they exit 1 with a "lands in a later task" message |
| No `lint` command | `lint` checks a config repository against the header contract |
| `bootstrap.sh` is a ~15-line fetcher | `bootstrap.sh` also checks the platform and caches the verified download |

| Doc | Contents |
|---|---|
| `spec.md` | Goal, non-goals, architecture, screens, phases, verification |
| `implementation-spec.md` | Build-ready CLI and TUI spec: commands, screens, backend trait, scenarios |

## Decisions

- The engine stays `mise bootstrap`; TOML modules stay the source of truth for packages.
- The module list and descriptions are declared in `#:` header comments of
  the module files (`config.<name>.toml`), not in shell arrays.
- The crate lives in this repository at `apps/essential-mise-console/` and
  depends on `junie-tui` by pinned git revision, never a branch.
- Distribution: prebuilt release binaries; `bootstrap.sh` is a small fetcher
  that verifies a pinned checksum and execs the binary (`setup`).

## Open questions

1. Unapply scope: recommended for a later phase.
2. Visual identity: functional first, review later.
3. Persistent selection editing: read-only hint first, editor later.
