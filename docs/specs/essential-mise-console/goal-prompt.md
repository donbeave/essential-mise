# Goal prompt — essential-mise-console implementation spec (CLI + TUI)

Paste into a fresh session (or `/goal`) to produce the build-ready spec:

/goal: write the build-ready implementation spec for essential-mise-console
(CLI + TUI) in ~/.config/mise/docs/specs/essential-mise-console/.

CONTEXT (verified 2026-11-04, re-verify before citing):
- essential-mise repo (this repo): mise config repo, env modules in
  config.<name>.toml, interactive flow in bootstrap.sh (after first-mac.sh
  stages 0-5). Read bootstrap.sh, config.toml, 2-3 config.*.toml modules,
  docs/manual-actions.md fully before designing anything.
- Component library checkout: /tmp/tcb
  (tailrocks/terminal-components-baseline, branch
  feature/openshell-native; re-fetch, it may be stale). Package
  `junie-tui`, lib `junie_tui`; precedent consumer apps/jackin
  (jackin-console). Crate name "termrock" does not exist — it is the
  design-system name; cite real paths.
- Locked decisions (see README.md + spec.md §10, Q1 resolved 2026-11-04):
  in-repo crate in essential-mise; junie-tui by pinned git rev, never a
  branch; TOML is source of truth; stock `mise bootstrap` stays the
  engine, the console only shells out (plan/status --json, apply); P0 bash
  per-package deselect ships before any Rust; P1 starts after baseline
  PR #1 merges. Toolchain: Rust 1.99, edition 2024, clap for CLI,
  ratatui + junie-tui for TUI.

TEAM (spawn parallel workers, one role each):
1. Usage researcher — traces today's end-to-end flow (fresh Mac stages
   0-5, bootstrap.sh steps incl. 6/7 selection, re-runs, drift,
   verify-current-inventory.py, miserc.local.toml one-shot semantics)
   and lists every user-visible scenario the console must cover.
2. Library librarian — inventories the live /tmp/tcb checkout: every
   reusable widget/screen primitive with file:line citations, feature
   flags (cli/terminal), the external-channel/worker-thread pattern from
   apps/jackin, and what must NOT be copied per its AGENTS.md.
3. CLI designer — clap command tree (env select, package select/deselect,
   plan preview, apply, status/verify, rerun affordances), flags, exit
   codes, --json/--dry-run parity with mise semantics, scripting use.
4. TUI designer — every screen as ASCII wireframe + widget mapping (only
   inventoried components), keybindings, focus order, search/filter,
   progress/cancel/failure/rerun-command states, result + follow-ups.
5. TUI engineer — Backend trait, world/state model, event flow, error and
   cancellation semantics, schema-drift isolation (plan --json has no
   origin field; origin comes from TOML parsing).

RULES:
- Zero invented APIs/flags: every junie-tui reference needs a
  path:line in the live checkout; every mise flag needs `mise help`
  evidence. Findings that contradict earlier spec claims must be stated
  explicitly, evidence wins.
- No reimplemented install logic; TUI/CLI never write package
  declarations (one-shot installs stay unrecorded, matching
  bootstrap.sh).
- Then cross-review: each role adversarially reviews the draft from its
  own aspect and must surface at least one load-bearing issue or state
  none found with evidence. Revise until clean.

DELIVERABLE: implementation-spec.md with (1) goals/non-goals, (2) full
CLI reference with examples, (3) all TUI screens with wireframes +
behavior + transitions, (4) Backend trait + data model, (5) scenario
matrix incl. fresh install, noop re-run, partial failure + exact rerun
command, deselect-all edge, drift, cancel mid-apply, secrets prompts,
missing package managers, offline, (6) phased build plan with exit
criteria (P0/P1/P2), (7) verification plan: cargo nextest (never cargo
test), clippy, fmt, tuiscotti baselines, Tart VM + Cua real-install
evidence (never host Mac), (8) open questions. Keep it tight, no filler.
