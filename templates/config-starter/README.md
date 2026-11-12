# My workstation config

A starting point for an [essential-mise](https://github.com/donbeave/essential-mise)
config repository. This directory becomes `~/.config/mise` on your Mac.

## Layout

| Path | Purpose |
| --- | --- |
| `config.toml` | Shared mise settings. Its `#:` header names the repository. Not a module. |
| `config.<name>.toml` | One file per module. Its `#:` header describes it. Select with `mise -E <name>`. |
| `dotfiles/` | Files that module `[dotfiles]` entries copy into place. |
| `.gitignore` | Keeps `miserc.local.toml` and secrets out of git. |

`foundation` is a base module (`#: base = true`): setup applies it automatically and
the TUI does not list it. `terminal` is an ordinary module the TUI offers.

## Use it

1. Copy this directory to a new folder and run `git init`.
2. Edit the `#:` header of `config.toml`: set `name`. For every
   `config.<name>.toml` you add, start the file with a
   `#: description = "..."` line; the module name is the file name.
3. Check your work: `essential-mise-console lint --config-dir .`
4. Push it to a private repository and run the engine's `bootstrap.sh` with
   `--config OWNER/REPO`.

The full contract is in the engine's `docs/config-repo.md`.
