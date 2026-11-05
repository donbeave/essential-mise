# Flow comparison — today vs proposed CLI/TUI (user perspective)

Method: every "today" row cites `bootstrap.sh` lines or `docs/first-mac.md`
sections, read fully 2026-11-05. Proposal columns cite `implementation-spec.md`
sections. Verdicts: **better** / **parity** / **gap** (with phase).

## A. Fresh Mac, stages 0–5 (out of console scope — correctly)

| Today | CLI / TUI | Verdict |
|---|---|---|
| One curl command → fetcher (~15 lines) → `emc setup` owns CLT, Homebrew, mise, adoption, foundation (`bootstrap.sh:39-156` behavior reimplemented as orchestration calling official installers; `first-mac.md:5-28`) | `emc setup` + S1→S4 | **better** (progress UI, was silent) |
| Fail-closed checkout validation: non-worktree, bare, symlink, wrong origin (`bootstrap.sh:89-151`) | Scenario 15: same checks, `Notice` + exit 1 | **parity** |
| Foundation apply `mise -E foundation bootstrap --upgrade --update` (`:156`) | `emc -E foundation apply` / S1 pre-tick | **parity** |

## B. Select environments (step 6, `bootstrap.sh:158-221`)

| Today | CLI / TUI | Verdict |
|---|---|---|
| Numbered list of 29 envs **with** descriptions (`:198-200`), free-text `1,3-5`, validated on submit, deduped (`:207-221`); invalid → whole script exits 1 | S1: same names+descriptions parsed from the script's arrays (no drift), live search, per-env installed/drift badges, validation before continue | **better** (search + badges + no dead-submit) |
| No pre-check: cannot see noop vs change before answering | S1 badges (§3), `emc plan --detailed-exitcode` gates | **better** |
| Non-tty silently skips (`:203-205`) | Fail-closed: explicit subcommand required (§2) | **better** (deliberate change, honest) |
| No per-package visibility at select time | S2 checklist grouped by declaring module | **better** |

## C. Apply (step 6 tail, `:223-237`)

| Today | CLI / TUI | Verdict |
|---|---|---|
| `sudo -v` + 60s keepalive, then one `bootstrap --upgrade --update`; any failure kills the whole selection (`:226-231`) | S3 sudo pre-flight + keepalive; two-phase worker; stop with literal failing command; per-part progress | **better** |
| No progress UI, no log structure | StepRail + live log + progress bar + copyable rerun | **better** |
| `--upgrade --update` hardcoded on | Defaults on, visible toggles (§2, scenario 6) | **better** |

## D. Verify (step 7, `:239-247`)

| Today | CLI / TUI | Verdict |
|---|---|---|
| `zsh -lic` checks git/brew/mise/mas + `foundation doctor` only; selected categories explicitly unverified (`:244-246`) | `emc verify` = `status --json` + `plan` per selected env; doctor demoted to hint; S4 per-part table + re-check | **better** (closes the documented gap) |
| One-shot notice printed; users still expect persistence (`first-mac.md:28-30`) | S2/S3 "unrecorded one-shot" labels + P1 read-only persistence hint | **better**, partial (editor is P2) |

## E. Day-2 workstation flows (`first-mac.md:32-84`)

| Today | CLI / TUI | Verdict |
|---|---|---|
| Standard set with muscle-memory env (`-E foundation,terminal,development,security`) + `--dry-run` first (`:42-45`) | `emc -E … apply --dry-run` then `apply`; same env names | **parity** |
| MAS-signed-out: apply subset, sign in, rerun full set (`:47`) | S3 names the state + rerun command; S4 drift until signed in (row 12) | **better** (guided instead of remembered) |
| One module/runtime: apply, `exec --` one-shot, persist via edit (`:63-80`) | Apply covered; **`exec --` has no console equivalent** | **gap** (minor: stock `mise exec` stays available; add passthrough or document) |
| `miserc.local.toml` create + add-without-dropping (`:49-55`) | P1 read-only hint only | **gap** (P2 editor; biggest P1 functionality gap vs docs) |

## F. Update / upgrade (`first-mac.md:86-112`)

| Today | CLI / TUI | Verdict |
|---|---|---|
| Three separate intents: config `git fetch/diff/pull`, `--update` metadata, `--upgrade` packages (`:90-110`) | `--update`/`--upgrade` toggles; git checkout ops stay manual | **parity** (correct split) |
| Hook-owned six upgrade on apply; review formulas/signatures/permissions after (`:112`) | Engine passthrough, unchanged | **parity** |
| "Config checkout behind origin" is a manual `git fetch` observation | No console surface | **gap** (opportunity: S1 banner when checkout is behind; P2 candidate) |

## G. Removal (`first-mac.md:114-156`)

| Today | CLI / TUI | Verdict |
|---|---|---|
| `unapply <env> --dry-run/--yes` + per-env Fish helper table + per-agent plugin commands + prune preview with hook-owned exclusions (`:118-156`) | All P2 (rows 10–11); agent-plugin commands never in console | **gap, acknowledged** (P2 must cover the Fish table + exclusion list, not just `unapply`) |

## H. Manual actions & sysprefs (`manual-actions.md`, `first-mac.md:57`)

| Today | CLI / TUI | Verdict |
|---|---|---|
| Hand-run `git config`/`gh auth`/`ssh`/Xcode/VPN/TCC commands from docs | S4 read-only surface, never auto-executed | **better** (surfaced in context) |
| system-preferences snapshot/rollback discipline | P2 | **gap, acknowledged** |

## I. Failure & troubleshooting (`first-mac.md:158-167`)

| Today | CLI / TUI | Verdict |
|---|---|---|
| MAS missing → sign in, claim, rerun (`:164`) | Row 12 | **better** |
| Existing-config refused → inspect status/toplevel/origin (`:161`) | Row 15 fail-closed | **parity** |
| Hook PENDING (exit 0, untouched) | `Pending` marks, never red (row 17) | **better** |
| Interrupted CLT/brew, Amp Desktop failure, AI auth, prefs rollback | Out of scope (pre-console or P2) | **parity by scope** |
| Offline / unreachable | Row 22: error on S3 log + rerun, no silent partial | **parity** |

## J. Scripting / non-tty

| Today | CLI / TUI | Verdict |
|---|---|---|
| Long `MISE_CEILING_PATHS=… mise -C … -E …` incantations; `--missing`/`--detailed-exitcode` gates | Shorter `emc` equivalents; `--json` envelopes; same exit semantics (+ exit 3 partial) | **better** |

## Usability verdict (user perspective)

Genuinely better: search + badges before answering; per-package choice; preview;
live progress with a copyable rerun command; verify that covers what was
installed; shorter scripted commands. The proposal removes every documented
step-6/7 pain point without removing any capability in P1 scope.

Frictions to accept: two surfaces (CLI + TUI) to learn vs one script; four
screens vs one prompt for the repeat "install my usual set" run — mitigated by
non-interactive `emc apply -E … -y` parity and the P1 amendment below; TUI log
is capped while today's scrollback is infinite — mitigated by the log-spill
amendment below.

## Must-fix before P1 (new findings from this review)

1. **Q5 — distribution (RESOLVED, supersedes this item):** velnor release flow,
   pinned per-target artifact + sha256, fetcher + `emc setup` (see
   implementation-spec §8, plan T1.4). The `cargo install` path below is dead.
2. **P1 amendment — S1 pre-ticks from `miserc.local.toml` `env`** (read-only):
   bridges the one-shot trap and gives repeat users a one-keystroke path.
3. **P1 amendment — worker tees the full log to a file**, S3 shows the path:
   the 5k-line cap must never be the only record of a failed apply.
4. **P1 amendment — `exec` documented out-of-scope** with the stock command
   printed, or a thin passthrough; do not leave row E3 silent.
5. **P2 scope note — unapply screen must include** the Fish-helper table and
   hook-owned prune exclusions, not bare `unapply` (today's flow would regress).
