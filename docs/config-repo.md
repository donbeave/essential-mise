# Config repository contract

essential-mise is an engine. It installs and updates a Mac from a separate
**config repository** that you own. This page describes what that repository
must contain so the engine can use it.

## Layout

The root of the config repository is the mise global config directory
(`~/.config/mise`, or `$MISE_CONFIG_DIR`). The engine runs mise as:

```sh
MISE_CEILING_PATHS=<config_dir> mise -C <config_dir> ...
```

Typical repository. Only `config.toml` is required; every other entry is optional:

```text
config.toml                shared mise settings; its #: header names the repository
config.foundation.toml     a module (base)
config.terminal.toml       a module
bin/                       scripts referenced by hooks (optional)
dotfiles/                  files referenced by [dotfiles] entries (optional)
.gitignore                 must cover miserc.local.toml
```

There is no separate manifest file. The few facts the engine needs about your
repository live in special comments at the top of the config files themselves.

## Modules

A module is a file named `config.<name>.toml`. The module name is the part
between `config.` and `.toml`; it is never declared anywhere else. The engine
selects modules with mise environments (`-E <name>`). The user's persistent
selection is stored in the untracked file `miserc.local.toml`; the TUI pre-ticks
modules from it. `config.toml` and any `*.local.toml` file are never modules.

## Metadata comments (`#:`)

### The format

A metadata line is a line that starts at column 0 with `#:`, optionally
followed by one space, then TOML `key = value` text:

```toml
#: description = "Chat and video-call apps"
#: order = 40

[bootstrap.packages]
"brew-cask:slack" = "latest"   # pin-exception: Homebrew casks have no version pins
```

Rules:

- Only the **leading comment block** is read: everything before the first line
  that is neither blank nor a comment. Ordinary `#` comments and blank lines
  may be mixed in freely.
- The text after the prefix is joined line by line and parsed with the real
  TOML parser, so quoting, escapes, and types are TOML's. Duplicate keys are an
  error.
- Unknown keys are an error (`deny_unknown_fields`).
- A `#:` comment anywhere else (below the first line of TOML, trailing a value,
  or indented) is a lint error. It is never silently ignored. A `#:` line
  inside a multi-line string is string content, not a comment, and is left
  alone.
- Errors are reported as `path:line: message` with the line of the offending
  `#:` line. CRLF line endings and a UTF-8 byte-order mark are accepted.

The Rust structs in `src/metadata.rs` are the source of truth for the keys
below.

### `config.toml`: the repository

```toml
#: schema = 1
#: name = "my-workstation"
#: min_console = "0.2.0"
```

| Key | Type | Required | Meaning |
| --- | --- | --- | --- |
| `schema` | integer | yes | Metadata version. Must be `1`. |
| `name` | string | yes | Display name for the TUI header. Not empty. |
| `min_console` | `MAJOR.MINOR.PATCH` string | no | Oldest console version allowed to use this repository. |

`config.toml` must exist and carry this header.

### `config.<name>.toml`: a module

```toml
#: description = "Shell, prompt, fonts, and terminal utilities"
#: order = 10
```

| Key | Type | Required | Meaning |
| --- | --- | --- | --- |
| `description` | string | yes | One-line text shown beside the module in the TUI. Not empty, single line; surrounding whitespace is trimmed. |
| `order` | non-negative integer | no | Position in the TUI list. Lower comes first. |
| `base` | boolean | no, default `false` | A base module: applied automatically at the end of `setup` and not listed in the TUI. |

The TUI lists non-base modules sorted by `order` ascending, modules without an
`order` last, ties broken by module name. Leave gaps (`10`, `20`, `30`) so a
module can be inserted later without renumbering. A base module looks like:

```toml
#: description = "Tools every machine needs"
#: base = true
```

### Why it is hard to break

- **One place per fact.** The description of a module sits on line 1 of the
  file it describes. Adding, renaming, or deleting a module cannot leave a
  second file out of date, so "listed but missing" and "present but not listed"
  cannot happen.
- **No invented syntax.** The payload is plain TOML parsed by the TOML parser;
  there is no custom grammar to get subtly wrong.
- **Nothing is ignored silently.** Unknown keys, duplicate keys, a header
  below code, and `#:` lines in the wrong place are all lint errors with a
  file and line, so a typo never turns into a quietly missing module.
- **Invisible to mise.** To mise the header is a comment, so it cannot change
  what is installed.

## Example

```text
config.toml
config.foundation.toml
config.terminal.toml
config.messengers.toml
dotfiles/terminal/starship.toml
.gitignore
```

`config.toml`:

```toml
#: schema = 1
#: name = "my-workstation"
#: min_console = "0.2.0"

min_version = "2026.10.23"
```

`config.foundation.toml` is applied by `setup`, never shown in the TUI:

```toml
#: description = "Base tools applied to every machine"
#: base = true

[bootstrap.packages]
"brew:mas" = "latest"   # pin-exception: Homebrew formulae have no version pins
```

`config.messengers.toml` is listed after every module with a lower `order`:

```toml
#: description = "Chat and video-call apps"
#: order = 30

[bootstrap.packages]
"brew-cask:slack" = "latest"   # pin-exception: Homebrew casks have no version pins
```

## Hooks

Hooks in module files call config-owned scripts by this path, so they work from
any checkout location:

```text
${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}/bin/<script>
```

## Lint

```sh
essential-mise-console lint --config-dir DIR
```

Without `--config-dir`, lint checks `$MISE_CONFIG_DIR`, else `~/.config/mise`.
Exit code 0 prints `lint: <dir> is valid`. Exit code 1 prints one line per
problem, as `path:line: message` (or `path: message` for a whole file).

| # | Check | Example message |
| --- | --- | --- |
| 1 | `config.toml` exists and carries a valid header (`schema = 1`, non-empty `name`, optional `min_console`). | `config.toml: not found (it carries the repository header: schema and name)`; `config.toml:1: missing field `schema``; `config.toml:3: min_console "1.2" is not a MAJOR.MINOR.PATCH version` |
| 2 | Every `config.<name>.toml` carries a valid header with a non-empty, single-line `description`. | `config.extra.toml:1: missing field `description``; `config.extra.toml:1: missing metadata header: ...` |
| 3 | No header has unknown or duplicate keys, and no `#:` line sits outside the leading comment block or is indented. | `config.extra.toml:3: unknown field `icon``; `config.extra.toml:3: duplicate key ...`; `config.extra.toml:4: `#:` metadata line outside the leading comment block; move it above the first line of TOML` |
| 4 | Every `[dotfiles]` entry in a top-level `config*.toml` whose `source` is repo-relative points to an existing path. Sources that start with `~` or `/`, or contain `{{`, are skipped. | `config.terminal.toml: [dotfiles] "~/.x" source "dotfiles/x" does not exist` |
| 5 | Every `*.toml` file under the repository parses. Hidden directories, `node_modules`, and `target` are not searched. The `#:` lines are plain comments to TOML. | `dotfiles/x.toml: invalid TOML: ...` |
| 6 | `.gitignore` covers `miserc.local.toml`: an exact name, `/miserc.local.toml`, or a `*` wildcard such as `*.local.toml`. A negation (`!...`) or a directory pattern (`.../`) does not count. | `.gitignore does not cover miserc.local.toml` |

The check runs without `mise` and without network access. It validates the
contract, not the package names or the TOML schema that mise bootstrap reads.

## Adopting a config repository

`setup` needs to know where your config repository is. Give it one of these
forms with `--config`, or set `ESSENTIAL_MISE_CONFIG`:

| Form | Example |
| --- | --- |
| `OWNER/REPO` | `OWNER/REPO` |
| HTTPS URL | `https://github.com/OWNER/REPO.git` |
| SSH URL | `git@github.com:OWNER/REPO.git` |
| file URL | `file:///path/to/REPO` |

`OWNER/REPO` is shorthand for `https://github.com/OWNER/REPO.git`.

The value is required only when the config directory is missing or empty. If a
checkout already exists, `setup` checks that it is a non-bare worktree with an
`origin` remote. Uncommitted changes are allowed. If you also pass `--config`,
it must match `origin`. GitHub HTTPS and SSH forms compare equal when
`owner/repo` matches, ignoring case and a trailing `.git`.

## Setup stages

`setup` prints progress as `stage N/6`. Preview the plan with
`essential-mise-console setup --dry-run`; it spawns nothing.

| Stage | Name in the plan | What it does |
| --- | --- | --- |
| 1 | Apple Command Line Tools (Git) | Skipped when `xcode-select -p` and `git --version` succeed; otherwise runs `xcode-select --install` and waits. |
| 2 | Homebrew (latest) | Skipped when `/opt/homebrew/bin/brew` exists; otherwise runs the official installer. Then `brew update`. |
| 3 | mise (latest stable) | Runs the official installer from `https://mise.run`, then `mise self-update`. |
| 4 | Repository access | Probes the repository with `git ls-remote`; see below. |
| 5 | Adopt your configuration repository | Runs `mise bootstrap --adopt <url>` when the config directory is missing or empty; otherwise validates the existing checkout. |
| 6 | Base modules | Reads the headers, enforces `min_console`, then runs `mise -E <base modules> bootstrap --upgrade --update` with `MISE_CEILING_PATHS=<config_dir>`. |

After stage 6 the module-selection TUI opens.

Stage 4 runs `GIT_TERMINAL_PROMPT=0 git ls-remote <url>`. If that fails for a
GitHub HTTPS URL, the engine installs `gh` with Homebrew, runs
`gh auth login --hostname github.com --git-protocol https --web` and
`gh auth setup-git`, then retries once. For SSH URLs it prints instructions and
stops. `file://` URLs skip the probe.

The headers are first readable after stage 5. A header problem or an unmet
`min_console` fails the run at stage 5, before any base module is applied.
When `min_console` is newer than the running console, the message tells you to
rerun the latest `bootstrap.sh`.

## Pinning

Pin every version your configuration can pin, and pin it exactly. A pinned
version makes a rerun on a new Mac produce the machine you reviewed, and makes
an upgrade a reviewable diff instead of a surprise.

| Do | Do not |
| --- | --- |
| `node = "24.11.1"` in `[tools]` | `node = "latest"` |
| An exact version for every tool backend that accepts one, such as `aqua:`, `github:`, `npm:`, or `cargo:`. | A prefix such as `"24"`, a range, or `latest`. |

Some package managers cannot pin. Homebrew publishes bottles only for a
formula's current version, and the Mac App Store installs only the current
version of an app. mise skips an entry such as `"brew:jq" = "1.7"` with a
warning, so for these entries `"latest"` is the only working value. To stay on
a major line of a formula, use a versioned formula name, for example
`"brew:postgresql@17" = "latest"`. Record each remaining exception in the
config. This convention is not enforced by the engine, and lint does not check
it:

1. Put a marker comment on the line of the entry:

   ```toml
   [bootstrap.packages]
   "brew:bat" = "latest"   # pin-exception: Homebrew formulae have no version pins
   "mas:497799835" = "latest"   # pin-exception: App Store apps cannot be pinned
   ```

2. List every exception, with its reason and how you review upgrades, in a
   file in the config repository, for example `PIN-EXCEPTIONS.md`.
3. Find unreviewed entries with `rg '"latest"' config*.toml | rg -v 'pin-exception:'`.
   Each line it prints is a pin that is missing and has no recorded reason.

The starter template follows this convention; see
[`templates/config-starter`](../templates/config-starter).

## Bumping `min_console`

Raise `min_console` in `config.toml` when a config change needs a newer console,
for example a new header key. Use the exact released version:

1. Find the version in the engine's
   [releases](https://github.com/donbeave/essential-mise/releases) and in
   `EMC_VERSION` of `bootstrap.sh`.
2. Edit the header: `#: min_console = "0.2.0"`.
3. Run `essential-mise-console lint --config-dir .`.
4. Commit and push. A Mac with an older console stops at stage 5 and is told to
   rerun the latest `bootstrap.sh`.
