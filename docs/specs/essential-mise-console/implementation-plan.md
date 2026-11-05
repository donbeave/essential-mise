# essential-mise-console — implementation plan (executor-strict)

For a literal-minded executor. Follow tasks in order; do not skip verification
steps; do not invent flags, APIs, or paths. Anything marked OPERATOR means stop
and hand off — never work around it.

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
- L2. Toolchain: Rust 1.99.1, edition 2024 (`/tmp/tcb/rust-toolchain.toml`).
  Crate ships its own `rust-toolchain.toml` (copy that file verbatim).
- L3. `junie-tui`: git dependency, rev = **merge commit of baseline PR #1**
  (tailrocks/terminal-components-baseline#1). Current branch HEAD `a4e81a4` is
  NOT the pin. Gate G1 (below) produces the rev.
- L4. Test pins (from live `/tmp/tcb/Cargo.toml`, re-verify at use):
  `tuiscotti` git rev `d8d54155947f6303a41dfd6079b53c0abc16f29a`,
  `tui-test` (=tuiscotti-tui-test) `=0.2.1`, `ratatui 0.30`, `clap 4.6` with `derive`.
- L5. Decisions (operator-overridable, proceed as locked): Q2 unapply=P2, Q3
  functional-first P1, Q4 read-only persistence hint P1, Q5 stage-0 stays
  curl+bash and console installs via `git clone <tag>` + `cargo install --path
  apps/essential-mise-console --locked` after `development-rust`.
- L6. Backend argv shape (observed flows): `mise -C <config-dir> -E <envs>
  bootstrap …` with `MISE_CEILING_PATHS=<config-dir>`; config dir resolves from
  `--config-dir`, else `$MISE_CONFIG_DIR`, else `~/.config/mise`.
- L7. Source specs: `implementation-spec.md` (behavior), `flow-comparison.md`
  §"Must-fix" amendments (S1 pre-tick from `miserc.local.toml`, log spill file,
  `exec` documented out-of-scope). Wireframes in §3 are normative for layout.

## 2. Gate G1 — baseline PR #1 merged (do this before any junie-tui code)

1. `git -C /tmp/tcb fetch origin && git -C /tmp/tcb ls-remote --heads origin`
2. If `feature/openshell-native` still exists and `main` lacks its merge: STOP.
   Do P0 + T1.1–T1.3 + T2.x (no UI deps) and wait.
3. If merged: `REV=$(git -C /tmp/tcb rev-parse origin/main)` after fetch; record
   REV in the task evidence. Use exactly this REV in `Cargo.toml`.
4. Done when: REV is a full 40-char sha on `origin/main`, evidence pasted.

## 3. P0 — bash per-package deselect (unblocked, no gate)

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
1. Copy the new block into `/tmp/p0-harness.sh` with stub `mise()` and `sudo()`
   functions that echo their argv to a log file; feed it scripted selections
   (normal, deselect-some, deselect-all, invalid input).
2. Assert: normal → both phases with right argv; deselect-some → phase 2 lists
   only kept; deselect-all → phase 2 absent; invalid → exit 1.
3. Paste the harness log. Delete `/tmp/p0-harness.sh` after.

### T0.3 — repo gates + commit
1. `bash -n bootstrap.sh`; `python3 tests/test_plugin_lifecycle.py` (if it fails
   for missing `/opt/homebrew/bin/jq`, paste the error and mark OPERATOR-VM).
2. Commit (`-s`), push. Done when: pushed, evidence pasted.

## 4. P1 scaffold (no G1 needed)

### T1.1 — crate skeleton
Create exactly: `apps/essential-mise-console/Cargo.toml` (package
`essential-mise-console` 0.1.0, edition 2024, rust-version 1.99, lib
`essential_mise_console` `src/lib.rs`, binary `src/main.rs`, `doctest = false`),
`rust-toolchain.toml` (verbatim copy of `/tmp/tcb/rust-toolchain.toml`),
`src/main.rs` (`fn main(){println!("essential-mise-console: UI lands in T4")}`),
`src/lib.rs` (empty), `src/backend.rs` + `src/world.rs` (empty modules with
`// T2`), `src/screens/mod.rs` (empty, `// T4, gated on G1`).
Deps now — exact lines (versions verified 2026-11-06; re-check before use):
`clap = { version = "4.6", features = ["derive"] }`,
`serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`.
NO `junie-tui`, NO `ratatui` yet.
Done when: `cargo build --manifest-path apps/essential-mise-console/Cargo.toml`
succeeds and `cargo --version` shows 1.99.1.

### T1.2 — hygiene gates
In the crate dir: `cargo fmt --check`, `cargo clippy -- -D warnings`.
Install nextest executor-locally only if missing:
`cargo install cargo-nextest --locked` (record version). Then `cargo nextest run`
(zero tests = pass). No shared-config changes. Done when: all three green, pasted.

### T1.3 — CI workflow (repo has no `.github/`; hand-written, deviation recorded)
Create `.github/workflows/console.yml` with EXACTLY this content (only the
toolchain version may be bumped to match L2 at write time):

```yaml
name: console
on:
  push:
    paths: ["apps/essential-mise-console/**", ".github/workflows/console.yml"]
  pull_request:
    paths: ["apps/essential-mise-console/**"]
jobs:
  gates:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.99.1
        with: { components: clippy, rustfmt }
      - run: cargo fmt --check
        working-directory: apps/essential-mise-console
      - run: cargo clippy -- -D warnings
        working-directory: apps/essential-mise-console
      - run: cargo install cargo-nextest --locked
        working-directory: apps/essential-mise-console
      - run: cargo nextest run
        working-directory: apps/essential-mise-console
```

(Prior spec's velnor rule
applies to the baseline repo, which has `.velnor/`; this repo has none, so a
minimal hand-written workflow is correct here.)
Done when: file created; live CI result arrives after push in T5.3.

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
Run `cargo nextest run`. Done when: green, pasted.

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
mock call log). `cargo nextest run` green.

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

### T4.6 — tuiscotti baselines
Create `tests/functional/main.rs` (+ `support.rs` PTY helper + `baselines/` dir),
mirroring the reference shape in `/tmp/tcb` (`tests/functional/main.rs`,
`tests/functional/support.rs` with `tui_test::{Session,…}`, baselines under
`tests/functional/baselines/` — re-list that dir at write time, do not assume
its contents); one baseline per screen (S1–S4) with MockBackend; pins per L4.
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
S1→S4 walk, VM recipe (Tart disposable macOS, synthetic accounts, never host),
what evidence to bring back (screenshots/Cua log). Do NOT run it. State
"blocked on operator" and stop.

## 9. Whole-goal definition of done

- [ ] P0 merged: deselect works, `test_plugin_lifecycle.py` passes, behavior
      unchanged when nothing deselected.
- [ ] CLI `--help` complete, exit codes 0/1/3 as specified, no invented flags.
- [ ] 4 screens match wireframes; shared filter/rerun modules exist once each.
- [ ] nextest + clippy + fmt + tuiscotti green locally and in CI.
- [ ] OPERATOR VM evidence recorded for a 2-module apply.
- [ ] No unmerged branches, no red CI, no invented APIs (every cite re-verified).
