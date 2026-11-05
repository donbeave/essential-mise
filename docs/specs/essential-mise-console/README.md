# essential-mise-console — spec index

Thin Rust TUI frontend over the essential-mise `mise bootstrap` engine,
built on Termrock (`junie-tui`) as a component library.

| Doc | Contents | Status |
|---|---|---|
| `spec.md` | Full spec: goal, architecture, screens, phases, verification | Draft for review |
| `implementation-spec.md` | Build-ready CLI+TUI spec (reviewed 2026-11-05, 11 findings integrated) | Ready for P0 / P1 kickoff |
| `flow-comparison.md` | 1:1 today-vs-proposal review from the user perspective + P1 amendments | Ready |
| `implementation-plan.md` | Executor-strict goal/plan/tasks (gates, commands, evidence per task) | Ready |
| `resume-prompt.md` | Prompt to restart this work in a fresh session | Ready |
| `goal-prompt.md` | Prompt to produce the build-ready CLI+TUI implementation spec | Ready |

## Decisions already locked

- Engine stays `mise bootstrap`; TOML modules stay the source of truth.
- P0 (bash per-package deselect in `bootstrap.sh`) ships before any Rust work.
- P1 starts on the merged `tailrocks/terminal-components-baseline` main tip (pinned rev `2e9e60f`, never a branch).
- In-repo crate in `essential-mise` (subdirectory TBD), depending on `junie-tui` by git rev (pinned merged commit, never a branch).
- Any PR created for this work is driven to merge promptly: small, green CI, no orphan stacks.

## Open questions (see spec §10)

1. Repo home + name — RESOLVED 2026-11-04: in-repo crate in `essential-mise` (subdirectory TBD), `junie-tui` by pinned git rev.
2. Scope of P1 screens (unapply screen in or out?).
3. Visual identity: adopt a D-style baseline review like jackin's D27, or ship unstyled first?
