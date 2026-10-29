# essential-mise

A plain, native [mise bootstrap](https://mise.jdx.dev/bootstrap.html) configuration for an Apple Silicon Mac. No Rust CLI, component framework, profile resolver, setup daemon, or custom orchestration.

**Status: PRE-RELEASE.** Static and isolated validation is complete, but the disposable-Mac release gate has not passed. The mutable `main` branch is not an end-to-end-tested release source. Use a reviewed commit-pinned launcher for any real setup.

## Requirements

- Native Apple Silicon, macOS **27.2 or newer**.
- Internet access and an administrator account for Apple Command Line Tools and Homebrew prompts.
- App Store sign-in for MAS applications, including 1Password for Safari.

The repository targets **mise 2026.10.23**. `config.toml` is intentionally inert and no default environment is committed, so adoption alone installs and configures nothing beyond adoption/foundation when `bootstrap.sh` explicitly selects it.

## First Mac

After the public repository is published, review `bootstrap.sh` and select a reviewed commit. Replace `<reviewed-commit>` with its full SHA, then run the immutable launcher:

```bash
reviewed_commit='<reviewed-commit>'
essential_mise_script="$(/usr/bin/curl -fsSL "https://raw.githubusercontent.com/donbeave/essential-mise/$reviewed_commit/bootstrap.sh")" && test -n "$essential_mise_script" && /bin/bash -c "$essential_mise_script"
```

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

## Optional modules

Examples:

```bash
cd "${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"

# On a fresh Mac, install only the Ruby prerequisite if preview reports Ruby 3+ is required.
# This does not install the other AI packages and allows tap evaluation during preview.
mise -E ai-tools,ai-cli bootstrap packages apply brew:ruby --yes

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

## Removal

Deselecting an environment does not uninstall it. Remove the environment from `miserc.local.toml`, keep its declaration for cleanup, then preview:

```bash
mise bootstrap unapply ai-cli --dry-run
mise bootstrap unapply ai-cli
```

Packages, repositories, and preferences require separate handling. Package pruning is a broad, separately approved maintenance operation and can affect unrelated Homebrew packages; never run it automatically after deselection.

## Repository status and boundaries

The development checkout is only for implementation/testing/publication. Once adopted, `$MISE_CONFIG_DIR` is the authoritative active config; edit and commit there.

Actual fresh-Mac installation, third-party taps/casks, App Store flows, FFF, OMP model routing, shell behavior, login items, and macOS preference behavior require the disposable-Mac release tests in [docs/validation.md](docs/validation.md). Unsupported behavior is marked MANUAL or DROP, not replaced with custom scripts.

- [Environments](docs/environments.md)
- [Manual actions and preference reversals](docs/manual-actions.md)
- [Exhaustive migration matrix](docs/migration-decisions.md)
- [Validation evidence and unverified release gates](docs/validation.md)
