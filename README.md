# essential-mise

A modular [mise bootstrap](https://mise.jdx.dev/bootstrap.html) configuration for an Apple Silicon Mac. It uses native mise resources wherever they work, with six narrowly documented real-Homebrew compatibility exceptions.

**Status: PRE-RELEASE.** The complete standard workstation has not passed its end-to-end release gate. Review the current `main` branch before setup; it changes over time.

## Requirements

- Apple Silicon running macOS **27.2 or newer**, in a native Terminal rather than Rosetta.
- Internet access and an administrator account for Command Line Tools and Homebrew prompts.
- App Store sign-in for Mac App Store applications. Accounts, licenses, privacy consent, and application onboarding remain manual.

The configuration targets **mise 2026.10.23**. `config.toml` contains only its minimum version; no optional environment is selected by default.

## Set up a new Mac

Review the current `bootstrap.sh`, then run:

```bash
essential_mise_script="$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/donbeave/essential-mise/main/bootstrap.sh)" && test -n "$essential_mise_script" && /bin/bash -c "$essential_mise_script"
```

This always downloads the latest launcher from `main`, and adoption uses the repository's default `main` branch. Stage 0 prepares Command Line Tools, Homebrew, pinned mise, the adopted checkout, and `foundation` shell activation only. It does not permanently select optional modules. See [First Mac setup](docs/first-mac.md) for prompts, recovery, and the App Store flow.

Set the configuration path in each new shell, then preview and apply the standard workstation:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E foundation,terminal,development,security bootstrap --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E foundation,terminal,development,security bootstrap
```

Every machine-setup example uses `mise -C` with `MISE_CEILING_PATHS`, preventing trusted projects in the caller's directory or above the adopted checkout from contributing resources. To persist this selection for future shells, create `miserc.local.toml` in the adopted checkout:

```toml
env = ["foundation", "terminal", "development", "security"]
```

When adding modules later, edit that list without discarding existing entries. `-E` replaces the persistent selection for one invocation; it does not uninstall anything.

## Install one block

The [environment guide](docs/environments.md) lists every module, its purpose, prerequisites, and manual steps. For example:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap status --missing
```

`ai-tools` contains shared search and development utilities, `ai-cli` contains terminal coding agents, and `ai-desktop` contains desktop applications. They are independent selections.

## Install or activate a runtime

These are separate operations:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E js bootstrap
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E js exec -- node --version
```

The first installs the declared Node/Bun stack; the second uses it for one command. Neither permanently selects `js`. Add `js` to the existing `env` list in `miserc.local.toml` for future shells and unrelated directories. The same rule applies to `development-rust` and `development-java`.

## Maintain or remove

- [Update the adopted checkout, upgrade packages, and troubleshoot](docs/first-mac.md#4-update-and-upgrade)
- [Upgrade or remove the six Homebrew exceptions](docs/homebrew-exceptions.md)
- [Remove modules and their owner-specific configuration](docs/first-mac.md#5-remove-a-module)

Deselecting a module does not uninstall it. Native unapply, Fish-block removal, application-owned settings, package removal, and macOS preference reversal are distinct operations. Do not run a broad native formula prune while hook-owned Amp CLI, FFF, or OMP must remain installed.

## Current boundaries

- Signed-in installation, purchases/claims, and reruns for the 21 Mac App Store declarations remain manual and unverified.
- Amp Desktop is provisioned for inventory completeness, but its known macOS 27.2 GUI startup failure means a working application is not claimed.
- Authenticated OMP routing/instruction loading and interactive Claude status-line rendering require manual verification.
- Account, license, privacy, privileged-extension, and application-specific setup are not automated.
- Preference stored values can be checked, but every UI effect and delayed selective reversal requires manual verification.

Independent clean installation of every module, an applied native package/cask upgrade, Homebrew-owned cask behavior during actual upgrade/prune, a real new-Terminal UI session, and the complete standard-workstation gate remain open release checks.

## Guides

- [First Mac setup, maintenance, removal, and troubleshooting](docs/first-mac.md)
- [Environments and package groups](docs/environments.md)
- [Real Homebrew compatibility exceptions](docs/homebrew-exceptions.md)
- [Manual actions and preference reversal](docs/manual-actions.md)
- [Claude Code status line](docs/claude-statusline.md)
- [Disposable Tart/Cua verification](scripts/vm/README.md)
