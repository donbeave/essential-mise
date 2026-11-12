# essential-mise-console - screens, keys and exit codes

This page describes the interactive console as shipped. It replaces the screen
sections of `spec.md` (section 5) and `implementation-spec.md` (section 3), which
are the pre-v0.2.0 design record.

## Flow

Four steps, named by verbs. The header reads `Step n of 4 · <name>`.

| Step | Name | What it does |
| --- | --- | --- |
| 1 | Choose modules | Pick the modules to set up. Nothing changes yet. |
| 2 | Review and confirm | See what will change, choose Install or Skip this time per package, then apply. |
| 3 | Applying | Runs the apply. Success moves on by itself. |
| 4 | Done, or Needs attention | Says whether the Mac matches, and what to do next. |

There are no `S1` to `S4` codes in the interface.

## Words

One table (`src/screens/help.rs`, shown by `?`). Internal ids are never shown.

- **module**: a bundle of apps and settings you can switch on.
- **area**: one kind of thing a module manages: Apps and packages, Command-line
  tools, macOS settings, Login items, Dotfiles, Login shell, and so on.
- States: **Up to date**, **Needs changes**, **Failed**, **Skipped**. A state the
  console cannot interpret is shown as "Could not check", never as a difference.
- Packages: **Install** or **Skip this time** (for this run only; not saved).

Every state is a glyph plus a word, never colour alone.

## Keys

The footer is generated from the keys that work on the current screen.

| Key | Meaning |
| --- | --- |
| Enter | The primary action, shown on the line above the footer |
| Esc | Go back, or close |
| q | Quit (asks only while changes are pending or an apply runs) |
| ? | Help: keys on this screen and the words used |
| Space | Toggle (select a module, Install or Skip this time) |
| / | Search (step 1) |
| a, n | All, none (step 1 modules; step 2 packages) |
| h | Show or hide already-installed packages (step 2) |
| d | Details: every pending change as setting, now, wanted (step 4) |
| e | Expand or collapse "All good" (step 4) |
| c | Copy the focused or primary command (OSC 52) |
| R | Check again (step 4) |
| f, arrows | Follow or scroll the live details (step 3) |

`B`, `D` and Backspace no longer do anything.

## Step 4

- A verdict line and exactly one primary action: **Finish and exit** when nothing
  differs, **Apply the N pending areas again** otherwise.
- Sections in order: **Needs your attention**, **Optional, for your information**
  (open a new shell, restart Dock and Finder, plugin hooks that were skipped),
  and a collapsed **All good (N areas)**.
- `mise bootstrap status --json` is an inventory: every configured entry is listed,
  already-set ones included. An area needs changes only when an entry really
  differs ("1 of 67 values differs: com.apple.dock tilesize (now 48, wanted 36)").
- Applying again runs only what differs: pending packages through
  `packages apply <specs>`, other areas through `bootstrap --only <names>` using
  an explicit status-key to `--only` table (`src/status.rs`). An area without an
  `--only` name makes the base phase run in full. A re-apply does not refresh and
  upgrade everything again. The command shown by `d` and copied by `c` is built
  from the same argv the backend executes.
- If the same items still differ after applying, the screen says so and offers
  **Finish anyway** instead of another round.
- The check runs off the UI thread: "Checking your Mac" with a spinner, then
  "Checked HH:MM:SS".

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Nothing differs, or you left before any work (steps 1 and 2) |
| 3 | Changes are still pending when you leave |
| 1 | The check failed, the apply stopped, or you cancelled (Ctrl+C) |

On exit the console prints the outcome, each pending change, the follow-up
commands and the exact command that applies the remaining changes to the normal
terminal, so they outlive the alternate screen.

## Layout and accessibility

- Verified at 100x30 and 80x24. At 80 columns the module detail pane moves under
  the list. The floor stays 72x20, with a message below it.
- Text is truncated only where a key shows the full text (`d`, `e`, wrapped
  command lines); long follow-up text and commands wrap.
- `NO_COLOR` selects the mono level; `TERM=dumb` (or `JUNIE_TUI_GLYPHS=ascii`)
  selects ASCII glyphs (`[ok]`, `[!]`).

## Test hooks

`EMC_MOCK=1` runs the console over `MockBackend`. Further switches, all test-only:
`EMC_MOCK_FAIL=packages|base`, `EMC_MOCK_DELAY_MS`, `EMC_MOCK_STATUS=differs|differs-then-clean`,
`EMC_MOCK_FOLLOW_UPS=1`, and `EMC_REVIEW_ARM_MS` (how long a freshly opened Review
screen ignores Enter; default 400). Visual baselines live in
`apps/essential-mise-console/tests/functional/baselines` and are approved with the
pinned `tuiscotti accept` command, never by hand.

## Follow-ups (out of scope for this change)

- Save the module selection to `miserc.local.toml`, so a repeat run starts from it.
- Lazy per-module health (a status check per focused module, cached), so step 1
  can show an honest per-module marker.
- Changing the apply phase order to match the documented `mise bootstrap` order
  (packages before files, repositories, dotfiles, defaults and tools).
- Handling an expired sudo ticket during a long session (a re-apply after the
  ticket expired stops at "Administrator access"; today it needs a restart).
- Confirm the exact per-entry state names `mise bootstrap status --json` uses for
  a value that differs. The console recognises `differs`, `unset`, `missing` and
  similar; any other state is shown as "Could not check" until it is added to the
  table in `src/status.rs`.
