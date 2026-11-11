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
essential-mise.toml        manifest (required)
config.toml                shared mise settings (not a module)
config.foundation.toml     a module
config.terminal.toml       a module
bin/                       scripts referenced by hooks (optional)
dotfiles/                  files referenced by [dotfiles] entries (optional)
.gitignore                 must cover miserc.local.toml
```

## Modules

A module is a file named `config.<name>.toml`. The engine selects modules with
mise environments (`-E <name>`). The user's persistent selection is stored in
the untracked file `miserc.local.toml`; the TUI pre-ticks modules from it.
`config.toml` and any `*.local.toml` file are never modules.

## Manifest: `essential-mise.toml`

The manifest is parsed strictly: unknown fields are an error. The Rust structs
in `src/manifest.rs` are the source of truth for this table.

```toml
schema = 1
name = "my-workstation"         # shown in the TUI header
min_console = "0.2.0"           # optional; console refuses to run when older

[setup]
base_modules = ["foundation"]   # applied at the end of setup

[[modules]]                     # TUI order = file order
name = "terminal"               # must match config.terminal.toml
description = "Shell, prompt, fonts, and terminal utilities"
```

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `schema` | integer | yes | Manifest version. Must be `1`. |
| `name` | string | yes | Display name for the TUI header. |
| `min_console` | `MAJOR.MINOR.PATCH` string | no | Oldest console version allowed to use this repository. |
| `setup.base_modules` | list of strings | yes | Modules applied at the end of `setup`, without being offered in the TUI. May be empty. |
| `modules[].name` | string | yes | Module name; unique across the manifest. |
| `modules[].description` | string | yes | One-line text shown beside the module in the TUI. |

Modules appear in the TUI in the order of the `[[modules]]` tables. A module
named in `base_modules` does not need a `[[modules]]` entry.

## Hooks

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

1. The manifest parses.
2. Every module in `[[modules]]` has a `config.<name>.toml`.
3. Every `config.<name>.toml` is listed in `[[modules]]` or in `base_modules`
   (`config.toml` and `*.local.toml` are ignored).
4. Every `[dotfiles]` entry with a repo-relative `source` points to an existing
   path. Sources starting with `~` or `/` are skipped.
5. Every `*.toml` file parses.
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
6. Apply `base_modules`

Stage 4 runs `GIT_TERMINAL_PROMPT=0 git ls-remote <url>`. If that fails for a
GitHub HTTPS URL, the engine installs `gh` with Homebrew, runs
`gh auth login --hostname github.com --git-protocol https --web` and
`gh auth setup-git`, then retries once. For SSH URLs it prints instructions and
stops. `file://` URLs skip the probe.

The manifest is read after stage 5, so `base_modules` and `min_console` are
checked between stages 5 and 6.
