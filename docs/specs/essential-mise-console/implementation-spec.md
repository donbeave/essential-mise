# essential-mise-console — implementation spec (CLI + TUI)

Draft for adversarial review. Sources: worker briefs 2026-11-05 (researcher,
librarian, CLI designer, TUI designer, TUI engineer) re-verified against live
`/tmp/tcb` (`a4e81a4`, `feature/openshell-native`) and `mise help` / live
`plan --json` / `status --json`. Evidence wins over this draft.

## 0. Corrections to prior spec (`spec.md`)

- "30 environments" is wrong: `bootstrap.sh:159-164` declares **29**. Module
  names + descriptions live in `environment_names` / `environment_descriptions`
  arrays (`bootstrap.sh:159+`); the console parses them, never hardcodes.
- No `first-mac.sh` exists. Flow is `bootstrap.sh` + `docs/first-mac.md`.
- No `verify-current-inventory.py` exists. P0 exit gate is `tests/test_plugin_lifecycle.py`.
- `plan --json` carries no origin field (keys exactly `{action,current,desired,id}`),
  confirmed twice. Origin is derived from TOML parsing — required, not optional.
- `plan` covers only accounts/packages/files/services/firewall/compose — **not**
  repos/dotfiles/shell/defaults/tools/hooks. Installed/drift badges from `plan`
  are package-scoped; full verify uses `status --json`.
- `DataGrid` lives in `src/widgets/grid.rs:382`, not `table.rs`
  (`table.rs:168` is the editable `DataTable` — wrong tone for Result).
- Canonical copy path is shell `Clipboard` (OSC52), `src/shell/clipboard.rs:35`
  (`copy` at `:83`), not `ViewportAction` (which is viewport-internal, `:181`).
- `StatusKind::{Ok,Fail,Warn,Pending,Running,…}` (`src/theme.rs:1766`),
  `Tone::{Normal,Secondary,Muted,Faint,Error,Warning,Success,Info,…}` (`:1748`).

**Review round 2026-11-05** (all five roles vs live sources; every finding below
is integrated into the body): badges must come from `status --json`, not `plan`
(dotfile/repo/hook drift is invisible to plan); empty keep-list must skip phase 2
(bare `packages apply` means *all*); S3 needs the `sudo -v` + keepalive pre-flight;
`packages --manager` is console-side (no `-m` on `packages status`); partial-apply
exit is 3, not 2 (2 already means plan would-change); `--only`/`--skip` are
mutually exclusive; S2 uses `ListBox` multi-select or `OptionList`, never
`Checkbox`-in-`ListBox`; copy key is `y` inside `InfoDialog`; filter + rerun
formatting are shared modules; `Backend` returns `StatusSnapshot` (never `Value`)
via an `ApplyHandle` owning the child; queue bound sits at the sender;
`origin_map` values are `Vec<env>`.

## 1. Goals / non-goals

Goal: replace `bootstrap.sh` steps 6/7 free-text selection with a clap CLI plus a
Ratatui TUI (modules → packages → apply → result) over stock `mise bootstrap`.
Non-goals (locked): no reimplemented install logic; TOML stays source of truth;
one-shot installs stay unrecorded (no `packages use` in P1); no stages 0–5
replacement; no `junie-tui` contributions in P1; no `unapply`/prune/persistent-env
editor/sysprefs-review screens in P1 (P2).

## 2. CLI reference (`essential-mise-console`, clap derive)

Env selection rides stock global `-E/--env` (repeatable, comma-separated —
observed on every help page), never a console flag. Every mise flag below was
observed in `mise help`.

```
emc [--mise PATH] [--config-dir DIR] [-E/--env …] [-y/--yes] [-v…|-q] <CMD>
  modules [--json] [QUERY]                 # list 29 envs + descriptions
  packages [-E …] [--manager M] [--json]# checklist data; origin parsed from
                                           # config.<name>.toml, console adds
                                           # `declared_by: [<envs>]`; --manager
                                           # is console-side (no -m on
                                           # `packages status` — observed)
  plan [-E …] [-J/--json] [--detailed-exitcode] [-j/--jobs]
  apply [-E …] [--skip PART,…] [--only PART,…] [-m M] [PKG…]
        [--update] [--upgrade] [-n/--dry-run] [-y]
        # --only/--skip mutually exclusive (clap conflicts_with, observed);
        # empty keep-list skips phase 2 — never bare `packages apply` (= all)
  status [-E …] [-J/--json] [--missing]
  verify [-E …] [--json]                   # status --json + plan per env;
                                           # `foundation doctor` as hint only
  tui [modules|packages|apply|result] [-E …]
```

- No-args: TUI modules screen when tty; explicit subcommand required when piped
  (fail-closed — deliberate new behavior, not a mirror: `bootstrap.sh:197-205`
  silently proceeds with nothing selected on non-tty).
- `apply` = phase 1 `bootstrap --skip packages [parts]` then phase 2
  `packages apply [PKG…]` (`manager:package` form, e.g. `brew:jq`; omitted names
  = deselected, config untouched; empty keep-list skips phase 2 entirely —
  bare `packages apply` defaults to *every* package, observed). Defaults
  `--upgrade --update` (matches `bootstrap.sh` step 6) with toggles. Part vocabulary for `--only/--skip`:
  `plugins,packages,accounts,files,services,firewall,compose,repos,dotfiles,`
  `mise-shell-activate,macos-defaults,macos-launchd-agents,linux-systemd-units,`
  `user,tools,task,final-hook`. Exit 0 ok; 1 failed part (stop at first error —
  stop semantics need a live failure test before final wording; print literal
  failing command incl. `-E` list); 3 partial (phase 1 ok, phase 2 failed;
  2 is taken by plan would-change).
- `--json` = `-J` passthrough; console envelope
  `{"envs":[…],"resources":[…],"summary":{…}}` on stdout, human text on stderr.
  `plan --json` shape: `{"resources":[{"id":{"kind","name"},"current","desired",`
  `"action"}],"summary":{"create","update","remove","unchanged","unknown"}}`;
  `action==noop` marks package rows only (labeled "packages" — S1 installed badges
  come from `status --json`, §3); `unknown` (manager absent) warns unless
  the package/manager was named explicitly (observed apply semantics).
- `-n/--dry-run` (observed on `bootstrap`, `packages apply`, `unapply`): pure
  preview, exit codes preserved; maps to TUI Preview, never implies `--yes`.
- `status --missing` exits 1 on drift (lists all); `plan --detailed-exitcode`
  0 clean / 2 would-change / 1 error-or-unknown. `-j/--jobs -q --silent --locked
  -C/--cd --raw` pass through. `verify` additionally prints the `mise -E
  foundation doctor` hint (step 7 today checks only the login shell — the gap).
- P2, no stubs: `unapply <ENV>… [-f/--force] [-n] [-y]`, `packages use -e`,
  prune preview.

## 3. TUI screens (P1)

Global chrome: `PageHeader` (`src/widgets/page_header.rs:9`), `StatusBar`
(`:124`), `HintBar` (`src/widgets/hintbar.rs:67` + `HintKeys` `:70`),
`TerminalSession::enter/leave` + free `runtime::run` (`src/runtime.rs:208,249,363`)
and `external_channel()` (`:84`)
for worker-thread events as `Input::External` (`src/core/event.rs:31,41`;
wiring precedent `apps/jackin/src/app.rs:226,235-244,394` + test pump
`apps/jackin/tests/external.rs:27-40`; `cockpit.rs` is cited only for the
viewport cap below — it holds no external-channel code). Behavior shared twice
goes in a shared module, never per screen (Termrock rule, AGENTS.md).

S1 Modules — `ListDetail` (`src/widgets/list_detail.rs:158`, `ListRow:48`,
`.mark(StatusKind)` `:69`) + `TextInput` filter (`src/widgets/input.rs:55`);
detail pane `PropsList` (`src/widgets/props.rs:266`) + `DetailCard` (`:13`);
badges from `status --json` per-part state (`Ok`-equivalent→installed;
`plan action==noop` covers packages only — label it "packages", since plan is
blind to repos/dotfiles/shell/defaults/tools/hooks); no-match → `EmptyState` (`:44`); errors →
`Notice` (`src/widgets/notice.rs:42`). Keys: `↑/↓` move, `Space` toggle, `/`
filter, `a` all-filtered, `n` none, `Enter` continue (disabled at 0), `Esc`
quit-`Dialog` (`src/widgets/dialog.rs:46`).
```
┌─ essential-mise-console ─────────────────── 1/4 Modules ───────────────────┐
│ Filter: [neovim______________________]   showing 7 of 29 · 3 selected      │
│ ┌─ environments ───────────────────┐ ┌─ detail: neovim ──────────────────┐ │
│ │ ▸ [x] neovim            [ok]     │ │ Ghostty, shells, prompt, fonts,   │ │
│ │   [x] terminal          [ok]     │ │ and terminal utilities            │ │
│ │   [ ] tmux              [..]     │ │ status: installed (status --json) │ │
│ │   [ ] browsers          [drift]  │ │ packages: 12 declared / 12 kept   │ │
│ │   [ ] security          [..]     │ │ plan (packages): 12 × noop        │ │
│ └──────────────────────────────────┘ └───────────────────────────────────┘ │
│ [Continue →]   (disabled while nothing is selected)                        │
│ ↑/↓ move · Space toggle · / filter · a select-all · n select-none          │
└─ mise 2026.11.0 · status cached 4s ago ────────────────────────────────────┘
```

S2 Packages — one `ListBox` in multi-select mode (`src/widgets/list.rs:80`,
`SelectMode::Multi`, core toggle set as checked state) or grouped `OptionList`
(`:169`, `Item{group,detail,tag}`) — never `Checkbox`-in-`ListBox` (rows are
data `ListItem`, not widget slots; one `Checkbox` per package explodes the
focus ring). `Checkbox` is reserved for single-boolean prompts. Module group
headers non-focusable, `Panel::card(Some(name))` (`src/widgets/panel.rs:89,112`)
per declaring module; pre-ticked, counts `kept/total`. Filter is the shared
filter module (same as S1 — live substring, hidden rows keep state), not a
per-screen reimplementation. Keys: `Space` toggle, `A` keep-all, `N` keep-none, `/` filter,
`Enter` apply, `Bksp` back (S1 selection preserved). All-unticked → "Apply base
only (`--skip packages`)".
```
┌─ essential-mise-console ────────────────── 2/4 Packages ───────────────────┐
│ Filter: [rg_______________________] keeping 18 of 21 · neovim(12) shell(9) │
│ ┌─ neovim ───────────────────────────┐ ┌─ detail: ripgrep ───────────────┐ │
│ │ ▸ [x] ripgrep    brew      kept    │ │ origin: config.neovim.toml      │ │
│ │   [x] neovim     brew      kept    │ │ manager: brew · 14.x installed  │ │
│ │   [ ] tmux-plug  brew      omitted │ │ unticked = omitted from apply   │ │
│ ├─ shell ────────────────────────────┤ │ (never recorded in config)      │ │
│ │   [x] starship   brew      kept    │ │                                 │ │
│ └────────────────────────────────────┘ └─────────────────────────────────┘ │
│ [< Back]  [Apply →]  Space toggle · A keep-all · N keep-none · / filter    │
└─ deselection omits packages from `packages apply <kept…>` ─────────────────┘
```

S3 Apply — `StepRail` (`src/widgets/steps.rs:74`), `TextViewport`
(`src/widgets/viewport.rs:429`, `Span` `:27`, `.wrap(true).max_lines(5_000)` per
cockpit), `ProgressBar` (`src/widgets/progress.rs:586`), Cancel `Button::danger`;
failure modal `InfoDialog` (`src/widgets/info_dialog.rs:30`, copy key is `y`
inside the modal) with the exact rerun command verbatim + copy via `Clipboard`.
Rerun-command formatting is the shared module (same as S4 drift notes).
Pre-flight: `sudo -v` + 60s keepalive before phase 1 (mirrors `bootstrap.sh`
step 6 — privileged casks fail mid-apply without it), killed on done/cancel.
Worker runs phase 1 then phase 2 (skipped when keep-list is empty),
streams `WorkerMsg::Line/Done` via `ExternalSender::send` (`:57`); UI drains per
frame (`runtime.rs:542-545`), queue bound at the sender (byte cap + drop-oldest
with gap marker — the per-frame drain alone cannot bound a verbose run). Keys: `Esc`/`Ctrl-C` cancel
(`Dialog::destructive` confirm → `ApplyHandle::cancel()`, `ApplyOutcome::Cancelled`,
partial-apply warning + rerun affordance), `↑/↓` scroll, `c` copy outside the
modal, `Enter` on
done → S4. Hook PENDING routes render `Pending`, exit 0, untouched installs
(`docs/environments.md:75-105`) — never red.
```
┌─ essential-mise-console ──────────────────── 3/4 Apply ────────────────────┐
│ StepRail:  ● base ─────── ◐ packages (7/18) ─────── ○ verify               │
│ pre-flight: sudo -v + keepalive ✓                                          │
│ ┌─ log ──────────────────────────────────────────────────────────────────┐ │
│ │ ✓ mise -E neovim,shell bootstrap --skip packages --upgrade --update    │ │
│ │ ✓ base parts applied (files, repos, dotfiles, shell, tools)            │ │
│ │ ◐ packages apply: installing brew:ripgrep …                            │ │
│ │   packages apply: 7 of 18 done                                         │ │
│ └────────────────────────────────────────────────────────────────────────┘ │
│ Progress: ████████████░░░░░░░░░░░░  7/18        [Cancel]                   │
│ Esc/Ctrl-C cancel · ↑/↓ scroll log · c copy · Enter → Result when done     │
└─ worker thread → ExternalSender → Input::External · sender-side queue cap ─┘
```

S4 Result — auto `Backend::status` on entry; per-part table via `DataGrid`
(`grid.rs:382`) or `PropsList`; summary `StatusBar`; follow-ups `Notice` +
read-only `TextViewport` (login sessions, `docs/manual-actions.md` items —
surfaced, never auto-executed). Keys: `R` re-run `status --json`, `c` copy
summary (outside modals; `y` inside `InfoDialog`), `Enter/D` done, `B` back to S2. Drift → per-part note + suggested
`mise bootstrap …` command; `DiffView` (`src/widgets/diff.rs:204`) optional.
```
┌─ essential-mise-console ─────────────────── 4/4 Result ────────────────────┐
│ Summary: ✓ 5 of 6 parts ok · 1 drift                                       │
│ ┌─ part ────────┬─ state ─┬─ note ───────────────────────────────────────┐ │
│ │ base          │ ok      │ files, repos, dotfiles, shell all match      │ │
│ │ packages      │ drift   │ 2 packages pending (see rerun below)         │ │
│ │ tools         │ ok      │ rust 1.99.1, node pinned                     │ │
│ └───────────────┴─────────┴──────────────────────────────────────────────┘ │
│ follow-ups:  ① open a fresh login shell   ② gh auth login (manual-actions) │
│ rerun: mise -E neovim,shell bootstrap packages apply brew:foo              │
│ [Re-check]  [Copy summary]  [Done]   R re-check · c copy · B back to S2    │
└─ auto-ran `status --json` on entry · Done exits 0 only when all ok ────────┘
```

## 4. Backend trait + data model

```rust
pub struct PlanEntry { pub kind: String, pub name: String, pub current: String,
    pub desired: String, pub action: Action }
pub enum Action { Create, Update, Remove, Noop, Unknown }
pub struct StatusPart { pub part: String, pub state: String, pub drift: Vec<String> }
pub struct StatusSnapshot { pub parts: Vec<StatusPart>, pub raw_note: String }
pub struct ApplyLine { pub seq: u64, pub stream: Stream, pub text: String }
pub enum ApplyOutcome { Success, Failed { code: Option<i32>, tail: String }, Cancelled }
pub struct ApplyHandle { /* owns Child + worker JoinHandle */ }
impl ApplyHandle { pub fn cancel(&mut self) -> ApplyOutcome; pub fn join(self) -> ApplyOutcome; }
pub trait Backend: Send + Sync + 'static {
    fn plan(&self, envs: &[String]) -> Result<Vec<PlanEntry>, BackendError>;
    fn status(&self, envs: &[String]) -> Result<StatusSnapshot, BackendError>;
    fn spawn_apply(&self, req: &ApplyRequest, tx: ExternalSender)
        -> Result<ApplyHandle, BackendError>;
    fn origin_map(&self, config_dir: &Path) -> Result<HashMap<ResId, Vec<String>>, BackendError>;
}
pub struct ResId { pub kind: String, pub name: String } // declared_by: Vec<env>
pub enum BackendError { Spawn(io::Error), Parse(String), Timeout }
```

`backend.rs` owns all serde (tolerant: `#[serde(default)]`, no
`deny_unknown_fields`; `serde_json::Value` never crosses into screens — status
arrives as `StatusSnapshot`). World: `entries: Vec<Row>`
(`Row = PlanEntry + Option<origin>` joined client-side), pure view state
(selection/filter), `ApplyState::{Idle,Running{lines,handle},Done{…}}` (no
`child_pid` — cancel goes through `ApplyHandle::cancel()`, never pid-kill).
`Parse` errors show `schema mismatch…` and keep stale entries — never panic.
Mock backend returns fixture JSON for screenshot/unit tests. jackin wiring to
mirror: `with_external` (`apps/jackin/src/app.rs:226`), `Input::External →
on_external → downcast_ref` (`:394,235-244`); test pump
`apps/jackin/tests/external.rs:27-40`.

## 5. Scenario matrix

| # | Scenario | P | Console behavior |
|---|---|---|---|
| 1 | Fresh-Mac run (stages 1–5) | — | Out of scope (stages 0–5 untouched); console starts at module select |
| 2 | Category multi-select, search, skip | P1 | S1, 29 parsed envs, badges from `status --json` |
| 3 | Per-package deselect, grouped | P0+P1 | P0 bash prompt; P1 S2; empty keep-list skips phase 2 |
| 4 | Dry-run preview before apply | P0+P1 | `-n` in both |
| 5 | Apply, live log, native part order | P1 | S3 sudo pre-flight + two-phase worker |
| 6 | `--upgrade` vs missing-only vs `--update` | P1 | Separate toggles, defaults on |
| 7 | Re-verify selected envs | P1 | S4 status+plan; doctor hint only |
| 8 | Persistent-env (`miserc.local.toml`) | P2 | P1: read-only hint + edit instructions (one-shot trap warning) |
| 9 | One-shot vs recorded indicator | P1 | S2/S3 labels "unrecorded one-shot" |
| 10 | Unapply per env (+Fish owners, `--force`) | P2 | CLI stub forbidden; screen in P2 |
| 11 | Prune preview, hook-owned exclusions | P2 | — |
| 12 | MAS signed-out / claim-required / second-apply | P1 | S3 failure copy names the state, rerun cmd |
| 13 | Manual-action follow-ups | P1 | S4 read-only surface |
| 14 | system-preferences gated review | P2 | — |
| 15 | Dirty/wrong-origin/non-worktree/symlink config | P1 | Fail-closed `Notice` + exit 1 (mirror `bootstrap.sh:125-151`) |
| 16 | Non-tty | P1 | No TUI; explicit subcommand required; fail closed |
| 17 | Hook PENDING routes | P1 | `Pending` marks, exit 0 |
| 18 | Noop re-run (nothing to do) | P1 | S1 all-installed badges; `plan --detailed-exitcode` → 0; S4 all-ok, Done exits 0 |
| 19 | Cancel mid-apply | P1 | `Esc`/`Ctrl-C` → `ApplyHandle::cancel()`; partial-apply warning + exact rerun command; re-verify in S4 |
| 20 | Secrets prompts (`--prompt-secrets`) | P1 | Pass through in S3 log pane when tty; fail-closed with message when non-tty (blocks without tty, observed) |
| 21 | Missing package managers | P1 | `unknown` plan action → warning badge, never failure; error only if the manager is named via `--manager` or the package explicitly listed (observed apply semantics) |
| 22 | Offline / unreachable sources | P1 | Surface the mise error on the S3 log, stay on screen, exact rerun command; no silent partial success |

## 6. Phased build plan + exit criteria

- **P0 — bash deselect (hours, unblocked).** Second-level prompt in `bootstrap.sh`
  step 6: list selected envs' packages (from `[bootstrap.packages]` tables),
  untick, then `--skip packages` + `apply <kept>`; `sudo -v` + keepalive
  preserved. Exit: identical behavior when nothing deselected;
  `tests/test_plugin_lifecycle.py` passes.
- **P1 — console (days, after baseline PR #1 merges).** In-repo crate
  (subdirectory TBD); `junie-tui` by git rev pinned at the PR #1 **merge commit**
  (current `a4e81a4` is branch HEAD — do NOT pin it); CLI §2 + screens §3 against
  live JSON; `Backend` §4 with mock. Exit: fresh-VM dry run + 2-module apply via
  TUI; tuiscotti baselines for 4 screens; nextest + clippy + fmt green.
- **P2 — productization (weeks).** Unapply/prune/persistent-env/sysprefs screens,
  visual-identity review with approved baselines, full journey coverage, upstream
  generic widgets under baseline `AGENTS.md`, Tart-VM real-install evidence.

## 7. Verification

`cargo nextest` (never `cargo test`), clippy, fmt — same gates as baseline
(`.config/nextest.toml` pty group pattern per librarian). Visual: tuiscotti
screenshot baselines only (pinned rev per baseline `AGENTS.md`). Real installs:
Tart disposable macOS VM + Cua, synthetic accounts only, never host Mac. CI
generated (velnor pattern per prior spec); P1 gates on tuiscotti + nextest, VM
evidence reserved for P2/releases. PR hygiene (locked): any PR small, green, and
driven to merge — no orphans.

## 8. Open questions

- Review round 2026-11-05: all five roles adversarially reviewed the draft
  against live sources; 11 load-bearing findings integrated (see §0).
- Q1 Repo home — RESOLVED: in-repo crate, rev-pinned `junie-tui` (subdirectory TBD).
- Q2 Unapply scope — RECOMMENDED P2 (§5 rows 10–11: Fish owners, `--force`,
  hook-owned prune false-positives need their own design).
- Q3 Visual identity — RECOMMENDED functional-first P1, D-style baseline review P2.
- Q4 `miserc.local.toml` editing — RECOMMENDED read-only hint P1 (§5 row 8), editor P2.
- Q5 distribution — OPEN (from `flow-comparison.md`): stage 0 stays curl+bash;
  lock before P1 how the console installs (candidate: `cargo install --git`
  with pinned rev after `development-rust`, or a mise tool backend).
