# My workstation config

A starting point for an [essential-mise](https://github.com/donbeave/essential-mise)
config repository. This directory becomes `~/.config/mise` on your Mac.

## Layout

| Path | Purpose |
| --- | --- |
| `config.toml` | Shared mise settings. Its `#:` header names the repository. Not a module. |
| `config.<name>.toml` | One file per module. Its `#:` header describes it. Select with `mise -E <name>`. |
| `dotfiles/` | Files that module `[dotfiles]` entries copy into place. |
| `PIN-EXCEPTIONS.md` | Lists entries that cannot be version-pinned and why. A convention; the engine does not read it. |
| `.gitignore` | Keeps `miserc.local.toml` and secrets out of git. |

`foundation` is a base module (`#: base = true`): setup applies it automatically and
the TUI does not list it. `terminal` is an ordinary module the TUI offers.

## Use it

1. Copy this directory to a new folder and run `git init`.
2. Edit the `#:` header of `config.toml`: set `name`. For every
   `config.<name>.toml` you add, start the file with a
   `#: description = "..."` line; the module name is the file name.
3. Check your work: `essential-mise-console lint --config-dir .` (how to get the binary: the engine's `docs/create-your-config.md`).
4. Pin versions where the package manager allows it, and keep `PIN-EXCEPTIONS.md` in step with the `# pin-exception:` comments.
5. Push it to a private repository and run the engine's `bootstrap.sh` with
   `--config OWNER/REPO`.

The full contract, including pinning, is in the engine's [`docs/config-repo.md`](https://github.com/donbeave/essential-mise/blob/main/docs/config-repo.md).
