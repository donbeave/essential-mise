# essential-mise-console — spec

## 1. Goal

Replace the poor-terminal interaction of the original bash flow (numbered
category list, free-text input, no per-app choice, no progress UI, no
re-check) with a rich Ratatui TUI that lets the operator pick environment
modules, pick packages inside them, watch the apply run, and verify the
result — without reimplementing any install logic.

## 2. Non-goals

- No reimplementation of `mise bootstrap` (packages, dotfiles, repos,
  hooks, tools, unapply, plan/status). The engine stays stock mise.
- No new package format. `config.<name>.toml` modules remain the only
  source of truth for packages; the TUI never writes package declarations
  (one-shot installs stay unrecorded). The module catalogue comes from the
  config repository's `#:` header comments (see `docs/config-repo.md`).
- No replacement of VM verification scripts or docs. Stage 1–6 *orchestration*
  lives in `essential-mise-console setup` (calling official installers); install logic itself is
  never reimplemented. The TUI covers the setup + selection + apply + verify
  loop.
- No contribution to `junie-tui` in P1. Generic widgets go upstream only
  in P2, under that repo's `AGENTS.md` rules.

## 3. Background and constraints

- Depends on the Termrock component library (`junie-tui`) at a pinned git
  revision, never a branch. Upstream gaps go
  back as PRs with full review rounds.
- Upstream mise capabilities used (all verified against mise 2026.10.x):
  `bootstrap plan --json` → per-resource `{id, current, desired,
  action}`; `bootstrap status --json` → per-part state;
  `bootstrap --skip packages` + `bootstrap packages apply [PKG]…`
  (explicit subset installs unrecorded);
  `bootstrap packages use -e <env>`, `--only/--skip`, `--manager`,
  `status --missing`, `plan --json`, `unapply`.
- Per-package origin (which module declares a package) is derived by
  parsing the `[bootstrap.packages]` tables of the selected env files,
  not from mise output (current `plan --json` carries no origin field).
- Rust 1.99.1 / edition 2024 / ratatui 0.30 / crossterm 0.29 — already
  managed by essential-mise itself (dogfood).
- UI verification follows the host rules: tuiscotti baselines + nextest
  in CI, Tart VM + Cua for real-install verification.

## 4. Architecture

In-repo crate at `apps/essential-mise-console/`, one binary crate plus a small library
for the screen state, following the framework's recommended app/library
split:

```
apps/essential-mise-console/
  src/
    main.rs        # clap CLI, terminal setup, App wiring
    backend.rs     # Backend trait + mise-CLI implementation
    world.rs       # plain data: modules, packages, plan, run state
    screens/       # thin pages: modules, packages, apply, result
  tests/           # tuiscotti baselines + nextest unit tests
```

`junie-tui` is a dependency by exact git rev (pinned merged commit, never a branch; `default-features = false`
plus needed features, following the framework's recommended setup). Library code in our
crate never reimplements interaction; behavior shared twice moves to a
shared module, never duplicated per screen (the Termrock rule).

### Backend trait (draft)

```rust
trait Backend {
    fn modules(&self) -> Vec<Module>;                       // from config.<name>.toml files
    fn packages(&self, envs: &[Env]) -> Vec<PackageEntry>;  // with declaring-module origin
    fn plan(&self, envs: &[Env]) -> Plan;                   // `bootstrap plan --json`
    fn apply(&self, req: &ApplyRequest, out: &ExternalSender); // runs mise, streams events
    fn status(&self, envs: &[Env]) -> Status;               // `bootstrap status --json`
}
```

Long-running applies execute on worker threads and report through
`junie_tui::runtime::external_channel()` as `Input::External`
(progress lines, completion, errors) — the same pattern as the framework's
cockpit build log example. The apply screen renders the live stream in the
library terminal pane; cancellation forwards to the child process.

## 5. Screens (P1 scope)

> The shipped flow, keys and exit codes are in `screens.md`; this section is the
> original design.

1. **Modules** — multi-select list of the modules declared in the `#:` headers, with their
   descriptions; search filter; shows already-installed
   state where `plan --json` reports `noop`.
2. **Packages** — checklist of every package in the selected modules,
   grouped by declaring module, pre-ticked; untick to deselect.
   Deselected packages are simply omitted from the apply call.
3. **Apply** — runs `bootstrap --skip packages` for the full run, then
   `packages apply <kept…>`; live output in a terminal pane; progress
   per part; on failure, stops with the error and the exact rerun
   command.
4. **Result** — re-runs `status --json`, shows per-part ok/drfit,
   prints follow-ups (login sessions, manual actions listed by
   the config repository).

Out of P1 (P2 candidates): `unapply` screen, remote-host inventory,
`packages use` (record-to-config) flow, first-run adoption wizard.

## 6. Phasing and exit criteria

- **P0 — SUPERSEDED 2026-11-05** (no bash deselect; step-6 numbers stay until
  the first signed release; see implementation-spec §6 and plan §3).
- **P1 — thin Rust frontend (days, unblocked 2026-11-05).**
  In-repo crate, rev-pinned `junie-tui`, the 4 screens above against live
  mise JSON. Exit: fresh-VM dry run + apply of a 2-module selection
  through the TUI; tuiscotti smoke baselines for the 4 screens;
  nextest + clippy + fmt green.
- **P2 — productization (weeks).** Visual-identity review with
  approved baselines, full journey coverage, unapply/remote screens,
  upstream generic widgets if any, Tart-VM real-install evidence.

## 7. Verification

- Unit/in-process: `cargo nextest` (never `cargo test`), clippy, fmt —
  same gates as the baseline repo.
- Visual: tuiscotti screenshot baselines only; no other snapshot tool.
- Real installs: Tart disposable macOS VM + Cua, synthetic accounts
  only; never the host Mac. Compilation and dry runs alone do not
  establish runtime verification.
- CI generated by velnor-actions from `.velnor/config.toml`; never
  hand-edit `.github/workflows`.

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Termrock pre-1.0 API churn (merged as `2e9e60f`, still moving) | Pin exact rev (`2e9e60f`); monthly bumps per upstream policy; keep our usage to stable widgets (`Select`, `ListDetail`, `Dialog`, terminal pane) |
| `plan --json` schema drift (no `origin` field today) | Derive origin from TOML parsing (no mise dependency); isolate JSON parsing behind `backend.rs` with schema tests |
| TUI verification cost (VM time per change) | P1 gates on tuiscotti + nextest; VM evidence reserved for P2 and releases |

## 9. PR hygiene

Any PR created for this work is driven to merge promptly: small scope,
green CI (nextest + clippy + fmt + tuiscotti), no orphan stacks, no
unmerged PRs left behind.

## 10. Open questions

1. Repo home and name — RESOLVED 2026-11-04: in-repo crate in
   `essential-mise` (subdirectory TBD); `junie-tui` by pinned git rev.
2. Is the `unapply` screen P1 or P2? — RECOMMENDED P2 (see implementation-spec §8).
3. Visual identity: run a D-style baseline review before P1 ships, or
   ship functional-first and style in P2? — RECOMMENDED functional-first P1.
4. Should the TUI also replace the `miserc.local.toml` persistent-env
   editing, or stay one-shot like the original bash flow? — RECOMMENDED read-only
   hint P1, editor P2.
