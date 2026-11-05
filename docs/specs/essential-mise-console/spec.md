# essential-mise-console — spec

## 1. Goal

Replace the poor-terminal interaction of `bootstrap.sh` steps 6/7 (numbered
category list, free-text input, no per-app choice, no progress UI, no
re-check) with a rich Ratatui TUI that lets the operator pick environment
modules, pick packages inside them, watch the apply run, and verify the
result — without reimplementing any install logic.

## 2. Non-goals

- No reimplementation of `mise bootstrap` (packages, dotfiles, repos,
  hooks, tools, unapply, plan/status). The engine stays stock mise.
- No new config format. `config.<name>.toml` modules remain the only
  source of truth; the TUI never writes package declarations (one-shot
  installs stay unrecorded, matching today's `bootstrap.sh` semantics).
- No replacement of `first-mac.sh` stages 0–5, VM verification scripts,
  or docs. The TUI covers the interactive selection + apply + verify
  loop only.
- No contribution to `junie-tui` in P1. Generic widgets go upstream only
  in P2, under that repo's `AGENTS.md` rules.

## 3. Background and constraints

- Depends on `tailrocks/terminal-components-baseline` main tip `2e9e60f` (PR #1
  merged 2026-11-06). The crate pins that rev, never the branch. Upstream gaps go
  back as PRs with full review rounds (plan T6.1).
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
- Rust 1.99 / edition 2024 / ratatui 0.30 / crossterm 0.29 — already
  managed by essential-mise itself (dogfood).
- UI verification follows the host rules: tuiscotti baselines + nextest
  in CI, Tart VM + Cua for real-install verification.

## 4. Architecture

In-repo crate in `essential-mise` (subdirectory TBD), one binary crate plus a small library
for the screen state, following the `apps/jackin` (`jackin-console`)
split:

```
essential-mise-console/
  src/
    main.rs        # clap CLI, terminal setup, App wiring
    backend.rs     # Backend trait + mise-CLI implementation
    world.rs       # plain data: modules, packages, plan, run state
    screens/       # thin pages: modules, packages, apply, result
  tests/           # tuiscotti baselines + nextest unit tests
  .velnor/         # generated CI (never hand-edited)
```

`junie-tui` is a dependency by exact git rev (pinned merged commit, never a branch; `default-features = false`
plus needed features, mirroring `jackin-console`). Library code in our
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
(progress lines, completion, errors) — the same pattern as jackin's
cockpit build log. The apply screen renders the live stream in the
library terminal pane; cancellation forwards to the child process.

## 5. Screens (P1 scope)

1. **Modules** — multi-select list of the 30 environments with the
   `bootstrap.sh` descriptions; search filter; shows already-installed
   state where `plan --json` reports `noop`.
2. **Packages** — checklist of every package in the selected modules,
   grouped by declaring module, pre-ticked; untick to deselect.
   Deselected packages are simply omitted from the apply call.
3. **Apply** — runs `bootstrap --skip packages` for the full run, then
   `packages apply <kept…>`; live output in a terminal pane; progress
   per part; on failure, stops with the error and the exact rerun
   command.
4. **Result** — re-runs `status --json`, shows per-part ok/drfit,
   prints follow-ups (login sessions, manual actions from
   `docs/manual-actions.md`).

Out of P1 (P2 candidates): `unapply` screen, remote-host inventory,
`packages use` (record-to-config) flow, first-run adoption wizard.

## 6. Phasing and exit criteria

- **P0 — bash deselect (hours, unblocked).** Second-level prompt in
  `bootstrap.sh`: list selected envs' packages, untick, then
  `--skip packages` + `apply <kept>`. Exit: same behavior as today
  when nothing is deselected; `verify-current-inventory.py` still
  passes.
- **P1 — thin Rust frontend (days, unblocked 2026-11-06).**
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
2. Is the `unapply` screen P1 or P2?
3. Visual identity: run a D-style baseline review before P1 ships, or
   ship functional-first and style in P2?
4. Should the TUI also replace the `miserc.local.toml` persistent-env
   editing, or stay one-shot like `bootstrap.sh` today?
