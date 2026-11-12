# Architecture

essential-mise is split in two so that the public engine never contains personal data.

## Engine and config

| | Engine (this repository) | Config (your repository) |
| --- | --- | --- |
| Visibility | public | usually private |
| Contains | `bootstrap.sh`, the Rust console, VM harness, docs, starter template | `config.toml`, `config.<module>.toml`, `dotfiles/`, hook scripts, VM scenarios |
| Knows about | the `#:` header contract | its own modules, packages, and accounts |
| Changes when | the tooling changes | your machine setup changes |

The config repository is checked out as the mise global config directory (`~/.config/mise`, or `$MISE_CONFIG_DIR`). The engine is cloned anywhere else for development and is never placed inside the config directory.

## Data flow

1. `bootstrap.sh` (stage 0) downloads a pinned console release, verifies its SHA-256, and runs `essential-mise-console setup` with the arguments you gave it, including `--config OWNER/REPO`.
2. `setup` runs six stages: Command Line Tools, Homebrew, mise, repository access, adoption of the config repository, and the base modules (`#: base = true`).
3. The console reads the `#:` headers of the config files for the module catalogue and descriptions, never a hardcoded list.
4. You pick modules in the TUI. The console runs stock `mise bootstrap` with `-E <module>` selections as `MISE_CEILING_PATHS=<config_dir> mise -C <config_dir> ...`, so projects outside the config directory cannot contribute resources.
5. A persistent selection is stored in the untracked `miserc.local.toml` of the config checkout.

The console never reimplements install logic: packages, dotfiles, repositories, hooks, status, and unapply stay in mise.

## Trust model

- The launcher is the only unpinned piece you execute. It fetches a console by version and refuses a checksum mismatch.
- The config repository is code: its hooks run on your Mac. Adopt only repositories you control. An existing checkout is accepted only when it is a non-bare worktree whose `origin` matches the expected repository; otherwise setup refuses and changes nothing.
- Setup does not pull an existing checkout. Updating is an explicit, reviewable `git` operation.
- The engine stores no credentials. Repository access for private repositories uses `gh` or SSH keys that you provide.
- Never put secrets in a config repository; reference a secret manager instead.
