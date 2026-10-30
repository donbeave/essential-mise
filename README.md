# essential-mise

A plain, native [mise bootstrap](https://mise.jdx.dev/bootstrap.html) configuration for an Apple Silicon Mac. No Rust CLI, component framework, profile resolver, setup daemon, or custom orchestration.

**Status: PRE-RELEASE.** Static and isolated validation is complete, but the disposable-Mac release gate has not passed. The mutable `main` branch is not an end-to-end-tested release source. Use a reviewed commit-pinned launcher for any real setup.

## Requirements

- Native Apple Silicon, macOS **27.2 or newer**.
- Internet access and an administrator account for Apple Command Line Tools and Homebrew prompts.
- App Store sign-in for MAS applications, including 1Password for Safari.

The repository targets **mise 2026.10.23**. `config.toml` is intentionally inert and no default environment is committed, so adoption alone installs and configures nothing beyond adoption/foundation when `bootstrap.sh` explicitly selects it.

## First Mac

After the public repository is published, review `bootstrap.sh` and select a reviewed commit. Replace `<reviewed-commit>` with its full SHA, then run the commit-pinned launcher:

```bash
reviewed_commit='<reviewed-commit>'
essential_mise_script="$(/usr/bin/curl -fsSL "https://raw.githubusercontent.com/donbeave/essential-mise/$reviewed_commit/bootstrap.sh")" && test -n "$essential_mise_script" && /bin/bash -c "$essential_mise_script"
```

Only the downloaded launcher is pinned. `mise bootstrap --adopt donbeave/essential-mise` still adopts the repository's current default branch; this command does **not** pin or make the complete adopted configuration immutable. Review the adopted revision separately before applying optional modules.

This prepares only Command Line Tools/Git, official Homebrew, pinned mise, inert adoption, and `foundation` zsh activation. A failed/empty download, cancelled prerequisite, conflicting existing config, or unsupported OS fails closed. Rerunning skips adoption when a valid checkout exists and applies only `foundation`, never previously selected optional modules.

See [docs/first-mac.md](docs/first-mac.md) for the full startup and App Store two-pass flow.

## Standard workstation

Run from the adopted config directory:

```bash
cd "${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
mise -E foundation,terminal,development,security bootstrap --dry-run
mise -E foundation,terminal,development,security bootstrap
```

For first-time creation of a persistent selection only:

```bash
printf 'env = ["foundation", "terminal", "development", "security"]\n' \
  > "${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}/miserc.local.toml"
```

That command overwrites an existing selector; do not use it to add modules. Review and replace the list deliberately when changing it later. `-E` replaces the persistent selection for one command and does not uninstall anything.

Apply the nine authorized Git tuning settings and XDG-safe ignore override through the exact `git config` commands in [Manual actions](docs/manual-actions.md). They are intentionally not merged automatically, because generic INI parsing can delete ordinary user-added Git settings.

## Optional modules

Examples:

```bash
cd "${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"

# Shared CLI utilities and coding agents, with no AI desktop apps
mise -E ai-tools,ai-cli bootstrap --dry-run
mise -E ai-tools,ai-cli bootstrap
mise -E ai-tools,ai-cli bootstrap status --missing

# Java runtime and build tools
mise -E development-java bootstrap --dry-run
mise -E development-java bootstrap

# Optional macOS preferences, with stored-state and behavior review
mise -E system-preferences bootstrap --dry-run
mise -E system-preferences bootstrap
```

Every module is documented in [docs/environments.md](docs/environments.md).

### Approved specification changes

The user subsequently approved six narrowly scoped real-Homebrew package exceptions (Claude Code, Amp CLI, OMP, FFF MCP, Amp Desktop, and Surge) and the exact Claude status-line script/wrapper. These explicit changes supersede revision 7's native-only and excluded-status-line rules; they are not claims of native-only compliance. Each exception requires a demonstrated incompatibility and a documented owner, health check, upgrade path, and removal path in [Homebrew exceptions](docs/homebrew-exceptions.md). Native mise remains the declaration default. `AGENTS.md` distinguishes the user's Homebrew-first package-source preference from declaration ownership and permits another source only when Homebrew is unavailable or outdated.

Scoped v1 explicitly retains Amp Desktop in `ai-desktop` to satisfy the requested Amp inventory, while accepting that its package currently installs but does not launch on macOS 27.2. This is package provisioning only, not a working-GUI claim. Signed-in MAS installation, purchases/claims, and reruns are accepted as manual, unverified v1 boundaries; the standard workstation is not described as fully tested. Authenticated OMP routing/instruction loading and interactive Claude status-line rendering are also accepted manual verification boundaries. These decisions preserve the complete declarations without converting failed or unverified behavior into success, and they are not unrestricted public-release approval.

The Git data-preservation fallback supersedes automatic INI merging: the nine tuning keys and XDG-safe `core.excludesFile` override are applied manually through Git's own CLI. GitHub CLI settings and its `co` alias are applied through `gh` itself, honoring `GH_CONFIG_DIR` and custom XDG while preserving unrelated keys. Ghostty copy targets use native `{{ xdg_config_home }}` variants. Fish activation and Amp completion use additive, XDG-aware blocks because native Fish activation and dotfile edit targets have fixed paths. A successful empty Homebrew `shellenv` result is accepted as a valid already-initialized environment, superseding the earlier nonempty-output requirement. Preference stored-state and preservation checks are verified, but every affected UI behavior and delayed selective reversal remains a manual validation boundary with an exact immediate-rollback procedure in [Manual actions](docs/manual-actions.md).

### Claude Code status line

`ai-cli` installs an executable status-line script and a zsh wrapper. Every `claude` launch from zsh passes it through `--settings`; the script shows the directory, model and effort, context usage, and currently running subagents. See [Claude Code status line](docs/claude-statusline.md) for the exact wrapper, output, and verification evidence.

### Install versus activate a runtime

`mise -E js bootstrap` installs Node/Bun for that invocation; it does not persist `js`. Use `mise -E js exec -- node --version` for a one-off command. To keep a runtime selected for future shells and unrelated directories, add `js` to `miserc.local.toml` and rerun bootstrap. The same distinction applies to `development-rust` and `development-java`.

## Update an adopted configuration

After publication, review both the summary and complete diff before pulling:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
git -C "$config_dir" fetch origin
git -C "$config_dir" diff --stat HEAD..origin/main
git -C "$config_dir" diff HEAD..origin/main
git -C "$config_dir" pull --ff-only
cd "$config_dir"
mise bootstrap --dry-run
mise bootstrap
```

Stop for a dirty/conflicting checkout. Plain `mise bootstrap --update` is separate: it refreshes package metadata and declared repositories, not the global config checkout by itself.

## Upgrade native packages and tools

Configuration pulls, package upgrades, and removal are separate operations. Review the native upgrade plan, then apply it deliberately:

```bash
cd "${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
mise bootstrap --upgrade --dry-run
mise bootstrap --upgrade
```

`--upgrade` respects exact tool pins and package version constraints. Node, Bun, Rust, mr-boxington, and GraalVM therefore remain at their declared versions until the configuration changes; the Grok Bot declaration also stays at its pinned DMG URL and checksum. It does not upgrade mise itself, update declared repositories, refresh the adopted Git checkout, or restart services. Stage 0 continues to install and verify the explicitly pinned mise version.

Upgrade the six Homebrew exceptions only with their separate commands in [Homebrew exceptions](docs/homebrew-exceptions.md). Routine hooks ensure presence and health without upgrading outdated packages. Formula upgrades can change dependencies. Cask upgrades can replace application bundles, so review signatures, login items, and Accessibility/Screen Recording/Automation permissions afterward instead of assuming they carry over.

## Removal

Deselecting an environment does not uninstall it. Remove the environment from `miserc.local.toml`, keep its declaration for cleanup, then preview:

```bash
mise bootstrap unapply ai-cli --dry-run
mise bootstrap unapply ai-cli
```

Native unapply does not execute hooks. Remove the XDG-aware Fish blocks explicitly while keeping their helper and declarations available:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
/usr/bin/python3 "$config_dir/bin/fish-config.py" unapply activate
/usr/bin/python3 "$config_dir/bin/fish-config.py" unapply amp
```

Each command removes only its marked block and preserves unrelated Fish startup content. GitHub CLI's application-owned settings and aliases are not reversed by native unapply; review or change them through `gh config` and `gh alias` rather than deleting the whole config or `hosts.yml`.

Packages, repositories, and preferences require separate handling. A real prune dry run offered to remove working hook-installed Amp CLI, FFF, and OMP as well as unrelated Homebrew packages. **Do not apply a broad native formula prune while these hook-owned packages must remain installed.** Never prune automatically after deselection; review every candidate and use the explicit, separate Homebrew cleanup documented in [Homebrew exceptions](docs/homebrew-exceptions.md).

## Repository status and boundaries

The development checkout is only for implementation/testing/publication. Once adopted, `$MISE_CONFIG_DIR` is the authoritative active config; edit and commit there.

[Validation](docs/validation.md) records actual fresh-Mac installation, third-party packages, FFF, shell behavior, login items, and macOS preference tests, plus the accepted MAS, Amp GUI, and authenticated-service boundaries. Proven native-package incompatibilities use documented real-Homebrew hooks; other unsupported behavior is marked MANUAL or DROP, not hidden in custom installers.

- [Environments](docs/environments.md)
- [Real Homebrew compatibility exceptions](docs/homebrew-exceptions.md)
- [Manual actions and preference reversals](docs/manual-actions.md)
- [Exhaustive migration matrix](docs/migration-decisions.md)
- [Validation evidence and unverified release gates](docs/validation.md)
- [Replayable isolated stage-0 and XDG fixtures](docs/validation-fixtures.md)
- [Isolated Tart/Cua VM verification](scripts/vm/README.md)
