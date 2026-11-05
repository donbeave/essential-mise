# essential-mise-console — implementation plan (executor-strict)

For a literal-minded executor. Follow tasks in order; do not skip verification
steps; do not invent flags, APIs, or paths. Anything marked OPERATOR means stop
and hand off — never work around it.

## Goal (what you are building and why)

Build a clap CLI plus a Ratatui TUI (`essential-mise-console`) that replaces
`bootstrap.sh` steps 6/7 free-text category selection with: module multi-select
with search and installed/drift badges → per-package checklist grouped by
declaring module → live two-phase apply with progress, cancel, and a copyable
rerun command → verify screen re-running `status --json`/`plan --json` per
selected env. The engine stays stock `mise bootstrap`; TOML stays the source of
truth; one-shot installs stay unrecorded. Phase P0 (bash deselect) ships first
and works on fresh Macs; the Rust console (P1) targets operators with a working
setup. You are done when §9's checklist is fully checked with pasted evidence.

## 0. Executor rules (read first, obey always)

- R0. Workdir is the repo root. Resolve it once: `REPO=$(git rev-parse --show-toplevel)`.
  Never use `/tmp/tcb` as a dependency path; never reference `~` unexpanded.
- R1. Read-only unless the task says mutate. Read-only mise commands only:
  `mise help …`, `mise bootstrap plan/status --json`, `mise bootstrap dotfiles status`,
  anything with `--dry-run`. NEVER run apply/install/unapply/`packages use` on the
  host. Real applies happen only in the OPERATOR VM task.
- R2. Every factual claim about `junie-tui` must be re-grepped in `/tmp/tcb`
  first (`git -C /tmp/tcb fetch origin` if older than one day). Every mise flag
  must be re-checked in `mise help` first. Cite `path:line` or paste the help line.
- R3. End every task by pasting its evidence (command + output). A task without
  evidence is not done.
- R4. On any failure: stop, paste the exact error, do not retry with a different
  approach more than once. If blocked on a decision, credential, or the VM, stop.
- R5. Keep diffs minimal. One task = one commit (`git commit -s`, committer must be
  `Alexey Zhokhov <alexey@zhokhov.com>`). Push only when the task says so.
- R6. Cargo cwd rule (the repo root is NOT a workspace): run every bare
  `cargo <cmd>` with cwd `apps/essential-mise-console/`. From the repo root,
  always add `--manifest-path apps/essential-mise-console/Cargo.toml`, and for
  `cargo run` use `--manifest-path …` (never `-p`, which requires a workspace).

## 1. Locked inputs (do not re-decide)

- L1. Crate dir: `apps/essential-mise-console/` (mirrors baseline `apps/jackin`).
  Binary `essential-mise-console`, lib `essential_mise_console`. Safe: nothing in
  the repo walks the root (`tests/` uses temp dirs; verified 2026-11-06).
- L2. Toolchain: latest stable Rust, never below 1.99.1 (1.99.1 is latest stable
  per `mise ls-remote rust`, verified 2026-11-06; re-resolve at start, record it).
  Edition 2024. Crate ships its own `rust-toolchain.toml` (copy
  `/tmp/tcb/rust-toolchain.toml` verbatim, bump `channel` to the resolved latest).
  mise itself must be ≥2026.11.1 (`mise self-update --yes`, same command as
  `bootstrap.sh:87`; 2026.11.1 adds `bootstrap --save-env` and lockfile v4 —
  see T3.3). `mbx` (mr-boxington's cargo shim, `/tmp/tcb/mise.toml:12`) is a CI
  concern only (`compile_driver = "mbx"` in velnor config): local builds use
  plain cargo + rust-toolchain.toml; do NOT add `[wrappers.cargo]` to shared config.
- L3. `junie-tui`: git dependency, rev = **merge commit of baseline PR #1**
  (tailrocks/terminal-components-baseline#1). Current branch HEAD `a4e81a4` is
  NOT the pin. Gate G1 (below) produces the rev.
- L4. Test pins (from live `/tmp/tcb/Cargo.toml`, re-verify at use):
  `tuiscotti` git rev `d8d54155947f6303a41dfd6079b53c0abc16f29a`,
  `tui-test` (=tuiscotti-tui-test) `=0.2.1`, `ratatui 0.30`, `clap 4.6` with `derive`.
  CI generator pin (from `/tmp/tcb/mise.toml:10`): `github:tailrocks/velnor-new`
  `0.1.73`, binary `velnor-actions`.
- L5. Decisions (operator-overridable, proceed as locked): Q2 unapply=P2, Q3
  functional-first P1, Q4 read-only persistence hint P1 (T3.3 may promote
  `--save-env`), Q5 RESOLVED 2026-11-06: release binaries via velnor release
  flow; P0 SUPERSEDED 2026-11-06 (no bash selection logic; see §3).
- L6. Backend argv shape (observed flows): `mise -C <config-dir> -E <envs>
  bootstrap …` with `MISE_CEILING_PATHS=<config-dir>`; config dir resolves from
  `--config-dir`, else `$MISE_CONFIG_DIR`, else `~/.config/mise`.
- L7. Source specs: `implementation-spec.md` (behavior), `flow-comparison.md`
  §"Must-fix" amendments (S1 pre-tick from `miserc.local.toml`, log spill file,
  `exec` documented out-of-scope). Wireframes in §3 are normative for layout.

## 2. Gate G1 — baseline PR #1 merged (do this before any junie-tui code)

Task dependency map (nothing starts before its needs are evidenced done):
G1 → T4.1–T4.6. T1.0 → every P1 task (no toolchain evidence, no code). T1.1 → T1.2, T2.1, T1.4. T1.2 → T1.3. T1.3 → T1.4 (release needs CI). T2.1 → T2.2, T2.3, T3.1. T2.2 + T2.3 → T3.2,
T4.6. T3.1 → T3.2, T3.4, T5.1. T1.0 → T3.3 (read-only). T4.1 → T4.2–T4.5. All code tasks → T5.1 → T5.2 → T5.3.
T0.1 → T0.2 → T0.3 (independent of all P1 tasks).

1. `git -C /tmp/tcb fetch origin && git -C /tmp/tcb ls-remote --heads origin`
2. If `feature/openshell-native` still exists and `main` lacks its merge: STOP.
   Do T1.1–T1.3 + T2.x (no UI deps) and wait.
3. If merged: `REV=$(git -C /tmp/tcb rev-parse origin/main)` after fetch; record
   REV in the task evidence. Use exactly this REV in `Cargo.toml`.
4. Done when: REV is a full 40-char sha on `origin/main`, evidence pasted.

## 3. P0 — SUPERSEDED 2026-11-06 (do not implement)

Per user directive, no bash selection logic is built: `bootstrap.sh` step 6 keeps
its numbered list until the first signed release, then delegates to the release
binary (see T1.4). The T0.1–T0.3 tasks below are retained for reference only.
P0 exit gate (`tests/test_plugin_lifecycle.py`) still applies to any
`bootstrap.sh` touch.

### T0.1 — implement the second-level prompt
Inputs: `bootstrap.sh:207-237` (category validation → sudo → apply).
Steps:
1. After the `fi` closing category validation (line ~221), before the
   `if [[ -n "$selected_environments" ]]` block, insert a new block that, for
   each selected env, lists `config.<env>.toml` `[bootstrap.packages]` keys
   numbered from 1, then one `read` reusing the exact `spaces or commas` syntax
   and validation idiom of lines 207-221 (digits only, range-checked, deduped).
2. Kept packages become a space-separated `manager:package` list; deselected are
   omitted. NOTHING is written to any TOML (one-shot semantics).
3. Phase split: `mise -E … bootstrap --skip packages --upgrade --update`, then —
   only if the keep-list is non-empty — `mise -E … bootstrap packages apply
   <kept…>`. Empty keep-list skips phase 2 (bare `packages apply` means ALL).
4. Keep `sudo -v` + 60s keepalive exactly as-is (lines 226-234).
Forbidden: changing validation semantics, touching stages 1-5/7, `packages use`.
Done when: `bash -n bootstrap.sh` passes AND the new block works in the harness (T0.2).

### T0.2 — harness-test the new block (no host mutation)
Write `/tmp/p0-harness.sh` with EXACTLY this scaffold, then paste the T0.1 block
in place of the `# <<< PASTE T0.1 BLOCK HERE >>>` marker (only the package-list,
package-read, and phase-split lines — not the sudo keepalive, which the stubs
cover):

```bash
#!/bin/bash
LOG=/tmp/p0-harness.log; : >"$LOG"
mise() { printf 'mise %s\n' "$*" >>"$LOG"; }
sudo() { printf 'sudo %s\n' "$*" >>"$LOG"; }
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
selected_environments="$SELECTED"
# <<< PASTE T0.1 BLOCK HERE >>>
```

Run four cases, resetting the log each time:
`SELECTED="terminal" ./p0-harness.sh <<<"1,2"` (normal),
`SELECTED="terminal" ./p0-harness.sh <<<"1"` after marking package 2 unticked
(deselect-some — adapt numbers to the fixture envs), `… <<<""` (deselect-all),
`… <<<"abc"` (invalid).
Assert: normal → both phases with right argv; deselect-some → phase 2 lists
only kept; deselect-all → no `packages apply` line at all; invalid → exit 1.
Paste the harness log. Delete `/tmp/p0-harness.sh` and the log after.

### T0.3 — repo gates + commit
1. `bash -n bootstrap.sh`; `shellcheck -S warning bootstrap.sh` (zero warnings;
   shellcheck rides mise shims per T1.0); `python3 tests/test_plugin_lifecycle.py`
   (if it fails for missing `/opt/homebrew/bin/jq`, paste the error and mark
   OPERATOR-VM).
2. Commit (`-s`), push. Done when: pushed, evidence pasted.

## 4. P1 scaffold (no G1 needed)

### T1.0 — toolchain bench (first P1 command sequence; everything P1 needs this)
1. `mise --version` → must be ≥2026.11.1, else `mise self-update --yes` and
   re-verify. Record the version.
2. `mise ls-remote rust | grep -E "^1\.[0-9]+\.[0-9]+$" | tail -1` → latest
   stable; must be ≥1.99.1. Record it; this is the `channel` for the crate's
   `rust-toolchain.toml`.
3. `command -v shellcheck && command -v cargo-llvm-cov` → both must resolve
   (via mise shims/aqua, as on this host). `cargo nextest --version` → if
   missing, T1.2 installs it. Record all versions.
4. `tart --version` → ≥2.41 (installed 2026-11-06 from the official
   `openai/tart` release tarball: the `cirruslabs/cli` Homebrew tap is broken
   at HEAD — `depends_on :macos` DSL error, exact message recorded in session
   history; bundle installed intact at `~/Applications/tart.app` with a
   `/opt/homebrew/bin/tart` shim, signature verified Notarized Developer ID).
   `cua-driver --version` → present via `cuadriver` Homebrew cask (0.38.0).
   Do NOT reinstall working tools; record versions only.
Done when: six version strings pasted (mise, rust, shellcheck, llvm-cov, tart,
cua-driver).

### T1.1 — crate skeleton
Create exactly: `apps/essential-mise-console/Cargo.toml` (package
`essential-mise-console` 0.1.0, edition 2024, rust-version 1.99, lib
`essential_mise_console` `src/lib.rs`, binary `src/main.rs`, `doctest = false`),
`rust-toolchain.toml` (verbatim copy of `/tmp/tcb/rust-toolchain.toml`),
`src/main.rs` (`fn main(){println!("essential-mise-console: UI lands in T4")}`),
`src/lib.rs` (starts with `#![warn(missing_docs)]`, then empty), `src/backend.rs` + `src/world.rs` (empty modules with
`// T2`), `src/screens/mod.rs` (empty, `// T4, gated on G1`).
Multi-crate direction rule (the `jackin-console` precedent: lib holds all logic,
bins hold wiring): `main.rs` does clap parse + terminal setup + App wiring ONLY
(anything else = reject); screens/state/backend live in the lib and never import
the binary crate. No second binary in P1.
Deps now — exact lines (versions verified 2026-11-06; re-check before use):
`clap = { version = "4.6", features = ["derive"] }`,
`serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`.
NO `junie-tui`, NO `ratatui` yet.
Done when: `cargo build --manifest-path apps/essential-mise-console/Cargo.toml`
succeeds and `cargo --version` shows 1.99.1.

### T1.2 — hygiene gates (strict from day one; baseline-observed + named extras)
In the crate dir, EXACTLY (R6):
`cargo fmt --check`,
`RUSTFLAGS="-D warnings" cargo clippy --all-targets -- -D warnings`
(`--all-targets` so tests are linted too; pedantic is explicitly OUT — noise
over value, decision recorded here, revisit in P2).
Install nextest executor-locally only if missing (T1.0 told you):
`cargo install cargo-nextest --locked` (record version). Then `cargo nextest run`
(zero tests = pass). No shared-config changes. Done when: all three green, pasted.

### T1.3 — CI via velnor-actions (never hand-write `.github/`)
CI is generated, not written. Exact steps:
1. Install the generator executor-locally (NO shared-config change in P1):
   `mise install "github:tailrocks/velnor-new@0.1.73"` (pin observed 2026-11-06
   in `/tmp/tcb/mise.toml:10`; re-check at use). Confirm `velnor-actions --version`
   runs via mise shims; record the version in evidence.
2. Read `/tmp/tcb/docs/ci.md` fully (3.4K) for discovery semantics. If velnor
   requires a Cargo workspace at the repo root, STOP and report — do NOT
   restructure the repo to please CI (our crate is standalone in `apps/`).
3. Copy `/tmp/tcb/.config/nextest.toml` verbatim to
   `apps/essential-mise-console/.config/nextest.toml` (pty group + functional
   profile; PTY tests flake without it).
4. Write `.velnor/config.toml` with EXACTLY this content (adapted from
   `/tmp/tcb/.velnor/config.toml`; only observed keys, nothing invented):

```toml
schema = 1

[workflow]
policy = "consumer-v1"

default_branch = "main"

runner_label = "ubuntu-26.04"
crate_timeout_minutes = 15

plan_timeout_minutes = 15

[stacks.rust]

compile_driver = "mbx"

test_runner = "cargo_nextest"

run_ignored = "default"

[discovery]

exclude = ["docs/**"]

[agents]

claude_md = "symlink"

[[stacks.rust.suites]]
id = "functional"
profile = "functional"
run_ignored = "default"
partitions = 1
build = "inline"
timeout_minutes = 15
test_threads = 4
required = true
triggers = ["pull_request", "push", "workflow_dispatch"]
artifacts = [{ name = "functional-terminal-evidence", paths = ["target/functional", "target/nextest/functional"], when = "failure", retention_days = 14 }]

[[stacks.rust.configurations]]
name = "default"
features = ["default"]
all_features = true
target = "host"
```

5. From the repo root run `velnor-actions generate`. Assert `git status`
   shows ONLY generated paths (`.github/`, `.velnor/`); anything else = stop.
   Commit the generated files. NEVER hand-edit anything under `.github/`
   (workflows, actions, scripts, velnor, actionlint.yaml, AGENTS.md/CLAUDE.md):
   change `.velnor/config.toml` and regenerate instead.
Done when: generated CI committed; live result arrives after push in T5.2
(green, or exactly one config-only fix + regenerate per R4, then stop).

### T1.4 — release pipeline (velnor release flow; gates the fresh-Mac path)
1. Re-read `/tmp/tcb/.velnor/release-manifest.json` at write time (pattern:
   versioned per-target artifacts + sha256, commit-pinned generator). Extend
   `.velnor/config.toml` with the release stanza for
   `essential-mise-console` on `aarch64-apple-darwin` ONLY (the single
   supported target; x86_64/Linux entries are forbidden until a VM exists to
   test them); regenerate; commit.
2. Cut the first release ONLY after T5.1 is green. Record artifact URL + sha256.
3. THEN, and only then, shrink `bootstrap.sh` to the fetcher: arch guard, download
   the pinned artifact to a mktemp path (reuse `run_official_installer` hygiene:
   curl -fsSL, non-empty check), verify sha256 against the pinned hash
   (hardcoded beside the version pin — two constants, updated together),
   `exec` the binary (which runs `setup`, then select/apply/verify).
   Everything else in `bootstrap.sh` is deleted. Any download/verify failure =
   `fail` (fail-closed; never fall through to partial state).
Forbidden: building releases per-commit (tag-gated only), `latest`-URL
consumption (pinned version always), touching step-6 selection semantics.
Done when: release published; bootstrap block merged; evidence = artifact URL,
sha256, and a fresh-VM install log (T5.3).

## 5. Backend + world (no G1 needed)

### T2.1 — `backend.rs` trait + live impl (read-only paths first)
Implement exactly the trait in `implementation-spec.md` §4 (`StatusSnapshot`,
`ApplyHandle`, `Backend: Send + Sync + 'static`, `origin_map → HashMap<ResId,
Vec<String>>`). Command construction per L6. `spawn_apply` runs phase 1 then
phase 2 (skip phase 2 on empty keep-list), streams `ApplyLine`, tees the FULL
log to a temp file whose path is returned with the outcome (flow-comparison
amendment 3). `sudo -v` + keepalive before phase 1; killed on done/cancel.
`origin_map` parses `config.<env>.toml` `[bootstrap.packages]` tables of the
requested envs only. Tolerant serde (`#[serde(default)]`, no `deny_unknown_fields`).
Done when: `cargo build` + `cargo clippy` clean.

### T2.2 — schema fixtures + unit tests
Save real outputs (redact nothing; they are local state, no secrets). The
fixtures dir does not exist yet — create it first or the redirects fail:
`mkdir -p apps/essential-mise-console/tests/fixtures`, then
`mise -C "$REPO" bootstrap plan --json > apps/essential-mise-console/tests/fixtures/plan.json`
`mise -C "$REPO" bootstrap status --json > …/status.json`.
Tests assert: entry keys exactly `{action,current,desired,id}`; no origin field;
unknown-action and missing-`summary` inputs parse without panic; multi-declare
origin yields `Vec` of length 2 in a synthetic TOML fixture.
Two tiers, no exceptions: tier 1 (fixtures, always run) for determinism; tier 2
(live read-only `plan`/`status` against the host config) behind `#[ignore]`,
run by operators via `cargo nextest run --run-ignored tier2` — CI never touches
the host config. Forbidden test shapes (reject on sight): no assertions,
`assert!(true)`, tests that only check mocks return what the mock was told,
snapshots of volatile fields (versions, timestamps).
Run `cargo nextest run`. Then `cargo llvm-cov --summary-only` (cargo-llvm-cov
per T1.0) and paste the report: every `Backend` method and every screen
transition in §3 needs at least one tier-1 test; uncovered ones are listed as
follow-up tasks, not waived.
Done when: green, pasted.

### T2.3 — mock backend
`MockBackend` returning the fixtures; used by all T4 screenshot tests.
Done when: compiles, one test loads plan fixture through the mock.

## 6. CLI (needs T2.1, no G1)

### T3.1 — clap tree per `implementation-spec.md` §2 verbatim
Commands `modules packages plan apply status verify tui`; global `-E` (stock
repeatable/comma form, never reimplemented); `--manager` on `packages` is
console-side filtering via `origin_map` (no `-m` passthrough — `packages status`
has no `-m`, verified); `--only`/`--skip` with `conflicts_with`; exit 3 =
partial apply; non-tty requires explicit subcommand (fail-closed).
`exec` stays out: print the stock `mise -E … exec -- …` command as a hint
(flow-comparison amendment 4).
Done when: `--help` output for every subcommand pasted; no invented flags
(diff each flag against `mise help`, one line of evidence per command).

### T3.2 — dry-run golden tests
`apply --dry-run` and `plan --json` against fixtures via MockBackend; assert exit
codes (0/1/3 mapping) and that `--dry-run` never spawns a child (assert on
mock call log). Same two tiers and forbidden shapes as T2.2 (tier-2 here =
`--help` text diffed against `mise help`, catching upstream flag drift).
`cargo nextest run` green.

### T3.3 — persistence primitive investigation (small, informs Q4)
mise 2026.11.1 added `mise bootstrap --save-env` (writes `-E` envs to
`miserc.local.toml`) and named `env_sets`. Read-only evaluation only:
`mise help bootstrap --save-env` (or parent help), plus `--dry-run` if the flag
supports one. Deliver a 10-line verdict in the task evidence: can the console's
persistence story shell out to `--save-env` (stock engine, no TOML writer of our
own), or does Q4 still need a P2 editor? Do NOT change the P1 read-only-hint
scope on a hunch — evidence first.

### T3.4 — `setup` subcommand (stages 1–5 orchestration in Rust)
Needs T2.1 (Backend spawning patterns), no G1. Implement `setup` per
`implementation-spec.md` §2: platform/arch guards, CLT trigger + `/dev/tty`
wait, official Homebrew + mise installers as subprocesses (same URLs/flags as
today's `bootstrap.sh:50-87` — re-read at write time), adoption + foundation
via stock `mise bootstrap`, then continue into the module flow in-process.
All installer I/O streams to the S3-style log pane (reuse the worker pattern).
Fail-closed at every stage with the stage number in the error (mirror today's
`fail` messages). Host execution forbidden: develop against fixtures; first
real run is the T5.3 VM.
Done when: `setup --dry-run` prints the staged plan with zero subprocesses
spawned (assert via mock call log); `cargo nextest run` green.

## 7. Screens (G1 REQUIRED — do not start before gate evidence)

### T4.1 — shared modules first (Termrock rule)
`src/screens/filter.rs` (live-substring filter, hidden rows keep state — used by
S1+S2, implemented ONCE), `src/screens/rerun_fmt.rs` (exact-rerun-command
formatting — used by S3+S4, ONCE), `src/screens/chrome.rs` (PageHeader/StatusBar/
HintBar wiring). Forbidden: per-screen copies of either.
Done when: clippy clean; unit tests for filter keep-state.

### T4.2 — S1 Modules (wireframe §3 normative)
`ListDetail` + `TextInput` + `PropsList`/`DetailCard`; badges from
`status --json` (plan `noop` labeled packages-only); env names+descriptions
parsed from `bootstrap.sh` `environment_names`/`environment_descriptions`
(regex on the arrays — re-verify array location by grep, never hardcode);
S1 pre-ticks from `miserc.local.toml` `env` (read-only; flow-comparison
amendment 2). Keys exactly per §3.
Done when: tuiscotti baseline screenshot test passes (T4.6 pattern).

### T4.3 — S2 Packages
`ListBox` multi-select mode OR `OptionList` — NEVER `Checkbox`-in-`ListBox`
(unimplementable: rows are data, verified). Group headers non-focusable,
`Panel::card(Some(name))`. All-unticked → "Apply base only". Keys per §3.

### T4.4 — S3 Apply
StepRail + TextViewport (wrap, sender-side-bounded) + ProgressBar +
`Button::danger` cancel + `InfoDialog` failure modal (copy key `y` inside,
`c` outside) + copy via shell `Clipboard` (OSC52). Event flow exactly the
`app.rs`/`tests/external.rs` pattern (NOT cockpit.rs — it holds no channel
code, verified). Hook PENDING → `Pending`, exit 0.

### T4.5 — S4 Result
Auto `Backend::status` on entry; per-part table via `DataGrid` (`grid.rs:382`)
or `PropsList` (NOT `table.rs:168` `DataTable` — wrong tone, verified);
follow-ups read-only; `R` re-check, `B` back to S2, Done exits 0 only if all ok.

### T4.6 — tuiscotti baselines + interaction tests
Create `tests/functional/main.rs` (+ `support.rs` PTY helper + `baselines/` dir),
mirroring the reference shape in `/tmp/tcb` (`tests/functional/main.rs`,
`tests/functional/support.rs` with `tui_test::{Session,…}`, baselines under
`tests/functional/baselines/` — re-list that dir at write time, do not assume
its contents); one baseline per screen (S1–S4) with MockBackend; pins per L4.
Beyond screenshots: one interaction test per screen transition in §3 (key press
→ state change), driven through the `Input::External` drain pump exactly like
`apps/jackin/tests/external.rs:27-40` (re-read it at write time). Same
forbidden test shapes as T2.2.
Done when: `cargo nextest run` green and baselines committed.

## 8. Integration + handoff

### T5.1 — end-to-end dry run (host-safe)
`cargo run --manifest-path apps/essential-mise-console/Cargo.toml -- -E terminal plan --json` and
`apply --dry-run` against the live repo config; assert exit codes and that no
child mutated anything (`status --json` before/after identical — paste both
hashes: `… | shasum`).
### T5.2 — push + CI
Commit per task (missed commits: squash per task now, one commit each), push,
paste the CI workflow result link. Red CI = fix before proceeding (never leave
a PR/commit red; small fixes only).
### T5.3 — OPERATOR VM handoff (executor STOPS here)
Prepare `VM-HANDOFF.md` in the crate dir: exact 2-module selection, expected
S1→S4 walk, VM recipe (Tart disposable macOS 27.2 guest from the `baseline-27-2`
baseline VM, `admin`/`admin` synthetic account, Cua Driver automation, never host),
what evidence to bring back (screenshots/Cua log). Do NOT run it. State
"blocked on operator" and stop.

## 9. Whole-goal definition of done

- [ ] P0 SUPERSEDED (no bash selection logic); `bootstrap.sh` untouched except the
      T1.4 download+exec block after the first release.
- [ ] First signed release published (aarch64-apple-darwin + sha256 manifest).
- [ ] CLI `--help` complete, exit codes 0/1/3 as specified, no invented flags;
      T3.3 `--save-env` verdict recorded.
- [ ] 4 screens match wireframes; shared filter/rerun modules exist once each;
      `main.rs` is wiring-only; `#![warn(missing_docs)]` on the lib.
- [ ] nextest + clippy (`RUSTFLAGS="-D warnings"`, `--all-targets`) + fmt +
      tuiscotti green locally and in CI; llvm-cov report pasted, every Backend
      method and screen transition tier-1 tested; zero forbidden test shapes.
- [ ] Toolchain evidence recorded (mise ≥2026.11.1, latest stable rust, nextest,
      shellcheck, llvm-cov versions).
- [ ] OPERATOR VM evidence recorded for a 2-module apply.
- [ ] No unmerged branches, no red CI, no invented APIs (every cite re-verified).
