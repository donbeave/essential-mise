# Goal prompt — BUILD essential-mise-console

Paste into a fresh session to start implementation:

/goal: implement essential-mise-console by strictly following
`docs/specs/essential-mise-console/implementation-plan.md` in the essential-mise
repo (`~/.config/mise`, remote `donbeave/essential-mise`, branch `main`).

HOW TO WORK (non-negotiable, from the plan §0):
- Read plan §Goal, §0 rules R0–R7, and §1 locked inputs L1–L7 FIRST. Obey them
  always. Workdir is the repo root (`REPO=$(git rev-parse --show-toplevel)`).
- Execute tasks in dependency-map order (§2). P0 is SUPERSEDED — do not touch it.
  Gate G1 is SATISFIED (pin `2e9e60f`); re-verify the tip, never a branch.
- Read-only mise commands on the host (`plan`/`status --json`, `--dry-run`).
  NEVER apply/install/unapply on the host. Real installs happen only in Tart VMs.
- Re-verify every junie-tui cite in `/tmp/tcb` (fetch first) and every mise flag
  in `mise help` before using it. Cite `path:line` or paste the help line.
- End every task with pasted evidence (command + output). No evidence = not done.
- One task = one `git commit -s` (committer `Alexey Zhokhov <alexey@zhokhov.com>`).
  Retry a failure at most once, then stop and report.
- OPERATOR stops (T5.3 VM handoff, P2/T6.1 upstream PRs, any R7 gap): write the
  handoff, state "blocked on operator", do not work around it.

ENVIRONMENT (verified 2026-11-05, re-check in T1.0):
Tart 2.41.1 + stopped `baseline-27-2` (macOS 27.2) + cua-driver 0.38.0,
mise ≥2026.11.1 (self-update per T1.0), Rust latest stable ≥1.99.1,
shellcheck + cargo-llvm-cov + nextest present, ~1.8 TiB free.

LOCKED DEFAULTS (proceed; operator overrides only on explicit instruction):
Q2 unapply=P2, Q3 functional-first P1, Q4 read-only hint P1 (T3.3 judges
`--save-env`, no scope expansion on hunch), Q5 release binaries + fetcher +
`emc setup`, Rust-only product code, reuse-first R7, velnor-generated CI only.

DEFINITION OF DONE: plan §9, all boxes checked with evidence. Keep responses
short. Never leave a commit red or a PR unmerged.
