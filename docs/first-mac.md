# First Mac setup

**Pre-1.0:** review `bootstrap.sh` and the console release it pins (`EMC_VERSION`) before running them.

## 1. Run the launcher

Use a native Apple silicon Terminal on macOS **27.2 or newer**. Update an older image through System Settings first; this project does not upgrade macOS. You need internet access, an administrator account, and interactive access to Apple's and Homebrew's installation prompts. You also need a config repository: see [Create your config](create-your-config.md).

```bash
/bin/bash -c "$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/donbeave/essential-mise/main/bootstrap.sh)" bootstrap --config OWNER/REPO
```

The word `bootstrap` becomes `$0` of the launcher; everything after it is passed to the console's `setup` command. `--config` accepts `OWNER/REPO`, an HTTPS or SSH URL, or a `file://` URL, and falls back to `$ESSENTIAL_MISE_CONFIG`. It is required only when the config directory is missing or empty. Add `--dry-run` to print the six stages and their commands without running them; the launcher still checks the platform and downloads and verifies the console first.

The launcher does three things (`[1/3]` to `[3/3]`): it checks for Apple silicon and macOS 27.2 or newer, downloads the pinned console release into `${XDG_CACHE_HOME:-$HOME/.cache}/essential-mise/` and verifies its SHA-256 (a cached archive is verified again before reuse), then runs `essential-mise-console setup`. A checksum mismatch stops the run.

`setup` prints `stage N/6`:

1. Apple Command Line Tools (Git). Finish Apple's dialog when prompted.
2. Homebrew: the official installer, then `brew update`. This does not upgrade installed packages.
3. mise: the official installer, then `mise self-update` to the latest stable mise.
4. Repository access. For a private GitHub HTTPS URL the console installs `gh`, runs a web login, and retries once. For SSH URLs it prints instructions and stops.
5. Adoption of the config repository into the config directory. The headers are read here; an invalid header or an unmet `min_console` stops setup.
6. The base modules (`#: base = true`).

The module-selection TUI then opens. Select modules with Space, press Enter to review packages, and confirm the apply.

The default config directory is `~/.config/mise`; `XDG_CONFIG_HOME` changes its parent and `MISE_CONFIG_DIR` selects an explicit path. Unsafe aliases, symlinked path components, colon-containing paths, bare repositories, repository subdirectories, and a conflicting `origin` fail closed. A nonempty non-Git target is preserved and stops setup; move it aside first. Rerunning the launcher skips adoption, does not pull, and reopens the TUI.

## 2. Select modules persistently

TUI selections are installed once and are not remembered. To persist a selection for future shells, edit the untracked `miserc.local.toml` in the config checkout, keeping existing entries:

```toml
env = ["terminal"]
```

Set the path once per shell for the commands below:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E <module> bootstrap --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E <module> bootstrap
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E <module> bootstrap status --missing
```

Always use `mise -C` with `MISE_CEILING_PATHS` so trusted projects in or above your current directory cannot contribute resources. `-E` replaces the persistent selection for one invocation; it does not uninstall anything.

## 3. Update and upgrade

Setup never pulls an existing checkout. Review and fast-forward it yourself:

```bash
git -C "$config_dir" fetch origin
git -C "$config_dir" diff --stat HEAD..origin/main
git -C "$config_dir" pull --ff-only
```

Then preview and apply again as above.

## 4. Remove a module

Deselecting a module does not uninstall it. Preview and remove its owned resources with native unapply:

```bash
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" bootstrap unapply <module> --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" bootstrap unapply <module> --yes
```

Then remove the module from `miserc.local.toml`. Application-owned settings, cloned repositories, login items, and macOS preferences need separate review. A broad package prune can remove packages another module still needs; review the candidate list first.

## Troubleshooting

- **Interrupted Command Line Tools or Homebrew installation:** rerun the launcher; completed stages are skipped.
- **Existing configuration refused:** the target is not an empty directory, not a worktree, or has a different `origin`. Move it aside or fix `origin`; the engine never overwrites it.
- **Unexpected project resources appear:** you ran mise without `MISE_CEILING_PATHS`. Use the commands above.
- **Mac App Store packages are skipped:** sign in to the App Store, then rerun the apply.
- **`command not found: mise` in the first new zsh window:** known issue. The shell activation block runs before `~/.local/bin` is on `PATH`. Open a new shell, or run `export PATH="$HOME/.local/bin:$PATH"`. See [AGENTS.md](../AGENTS.md#known-issues).
- **`This configuration requires essential-mise-console X or newer`:** the config repository raised `min_console`. Rerun the latest `bootstrap.sh` from the command above.
