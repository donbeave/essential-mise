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

Minimal repository:

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
#: description = "Slack, Zoom, and other messengers"
#: order = 40

[bootstrap.packages]
"brew-cask:slack" = "latest"
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

## ## Hooks

Hooks in module files call config-owned scripts by this path, so they work from
any checkout location:

```text
${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}/bin/<script>
```

## Lint

```sh
essential-mise-console lint [--config-dir DIR]
```

Exit code 0 means the repository is valid. Exit code 1 prints one line per
problem. Checks:

1. `config.toml` exists and its header is valid (`schema = 1`, a `name`).
2. Every `config.<name>.toml` has a valid header with a `description`
   (`config.toml` and `*.local.toml` are not modules).
3. No header has unknown or duplicate keys, and no `#:` line sits outside the
   leading comment block.
4. Every `[dotfiles]` entry with a repo-relative `source` points to an existing
   path. Sources starting with `~` or `/` are skipped.
5. Every `*.toml` file parses (the `#:` lines are plain comments to TOML).
6. `.gitignore` covers `miserc.local.toml`.

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

`setup` prints progress as `stage N/6`:

1. Command Line Tools
2. Homebrew
3. mise
4. Repository access
5. Adopt config
6. Apply the base modules (`#: base = true`)

Stage 4 runs `GIT_TERMINAL_PROMPT=0 git ls-remote <url>`. If that fails for a
GitHub HTTPS URL, the engine installs `gh` with Homebrew, runs
`gh auth login --hostname github.com --git-protocol https --web` and
`gh auth setup-git`, then retries once. For SSH URLs it prints instructions and
stops. `file://` URLs skip the probe.

The headers are read after stage 5, so the base modules and `min_console` are
checked between stages 5 and 6.
