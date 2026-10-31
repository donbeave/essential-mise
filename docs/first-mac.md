# First Mac setup

**PRE-RELEASE:** the complete standard workstation has not passed its end-to-end release gate. Review the current `main` branch before installation.

## 1. Run stage 0

Use a native Apple Silicon Terminal on macOS **27.2 or newer**. Update an older factory image through System Settings first; this project does not upgrade macOS. You need internet access, an administrator account, and interactive access to Apple's and Homebrew's installation prompts.

Review the current `bootstrap.sh`, then run:

```bash
essential_mise_script="$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/donbeave/essential-mise/main/bootstrap.sh)" && test -n "$essential_mise_script" && /bin/bash -c "$essential_mise_script"
```

This command always downloads the latest launcher from `main`. First adoption also uses the latest configuration from `main`; an existing-checkout rerun skips adoption and does not pull. The workflow has no commit parameter. `bash -c` preserves interactive installer input and the nonempty-download check fails closed.

Stage 0:

1. Checks the platform, macOS version, and non-root user.
2. Installs or verifies Apple Command Line Tools and Git. Finish Apple's dialog when prompted.
3. Installs official Homebrew when absent and initializes its environment. A successful empty `shellenv` result is valid when Homebrew is already initialized.
4. Installs and verifies mise `2026.10.23` through the official installer.
5. Adopts the public configuration only when the target is absent or empty. Existing checkouts must be valid nonbare worktrees at the expected physical root with the expected origin.
6. Applies only `foundation`, installing the `mas` CLI and configuring shell activation, then verifies a fresh login/interactive zsh.

The default checkout is `~/.config/mise`; `XDG_CONFIG_HOME` changes its parent and `MISE_CONFIG_DIR` selects an explicit path. Unsafe aliases, symlinked path components, colon-containing paths (which cannot be represented safely in `MISE_CEILING_PATHS`), bare repositories, repository subdirectories, and conflicting origins fail closed. Rerunning stage 0 skips adoption and applies only `foundation`, even if optional modules are selected.

Open a fresh Terminal after success. Its printed next-step commands include the concrete `mise -C` path and a matching `MISE_CEILING_PATHS` boundary, so projects in the caller's directory or above the adopted checkout cannot contribute resources.

## 2. Apply the standard workstation

Set the path once in the current shell:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
```

Preview and apply:

```bash
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E foundation,terminal,development,security bootstrap --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E foundation,terminal,development,security bootstrap
```

If App Store sign-in is unavailable, first apply `foundation,terminal,development`, sign in, then rerun the complete standard set. Stage 0's `foundation` environment installs the `mas` executable before `security` declares its Mac App Store package. Purchases and claims are not automated.

For first-time persistent selection, create `$config_dir/miserc.local.toml`:

```toml
env = ["foundation", "terminal", "development", "security"]
```

Do not overwrite an existing selector merely to add a module. Edit its `env` list while retaining existing entries. Project `.miserc*` files, `MISE_ENV`, and command-line `-E` can override this selection.

Apply the nonidentity Git settings and XDG-safe ignore override with the exact `git config` commands in [Manual actions](manual-actions.md). They deliberately use Git's own writer to preserve user settings, repeated helpers, includes, comments, and formatting.

## 3. Apply one module or runtime

Find the module and its prerequisites in [Environments](environments.md). For terminal coding agents and their shared utilities:

```bash
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap status --missing
```

Use `ai-cli` without `ai-tools` when only the agents and their own declarations are wanted. Use `ai-desktop` separately for desktop applications. Amp CLI, OMP, FFF, Amp Desktop, Claude Code, and Surge have the ownership rules documented in [Homebrew exceptions](homebrew-exceptions.md).

For a runtime stack, distinguish three actions:

```bash
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E development-java bootstrap
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E development-java exec -- java -version
```

- The first command installs the declared Java stack without persistently selecting it.
- The second uses that environment for one command.
- To persist it, add `development-java` to the existing `env` list in `miserc.local.toml`, then rerun the selected bootstrap.

Verify persistent selection in a fresh shell from an unrelated directory. Homebrew dependencies can install their own Node or OpenJDK independently of mise's selected runtime.

OpenCode instructions follow `XDG_CONFIG_HOME`. If `OPENCODE_CONFIG_DIR` is set, OpenCode prefers that user-owned directory instead; essential-mise does not duplicate instructions there. Unset the override or maintain its `AGENTS.md` deliberately.

## 4. Update and upgrade

Configuration updates, native package upgrades, Homebrew exception upgrades, and removal are separate operations.

Review and update the adopted checkout:

```bash
git -C "$config_dir" fetch origin
git -C "$config_dir" diff --stat HEAD..origin/main
git -C "$config_dir" diff HEAD..origin/main
git -C "$config_dir" pull --ff-only
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" bootstrap --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" bootstrap
```

Stop for a dirty or conflicting checkout. The final command applies the persistent selection; use explicit `-E` to limit it. Plain `MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" bootstrap --update` refreshes package/repository metadata rather than updating this Git checkout.

Preview and apply native upgrades deliberately:

```bash
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" bootstrap --upgrade --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" bootstrap --upgrade
```

Exact Node, Bun, Rust, mr-boxington, and GraalVM pins remain fixed until their declarations change. The Grok Bot DMG URL/checksum is pinned too. To change a pin, edit the relevant `config.*.toml`, review the diff, and run the preview/apply cycle for that module. Native upgrade does not upgrade mise itself, pull the configuration, update declared repositories, or restart services.

Upgrade the six real-Homebrew exceptions only with their separate commands in [Homebrew exceptions](homebrew-exceptions.md). Routine hooks install missing packages and run health checks without upgrading installed packages. Review formula dependency changes, replaced cask bundles, signatures, login items, and privacy permissions after an upgrade.

## 5. Remove a module

Deselection alone does not uninstall anything. Remove the module from `miserc.local.toml`, keep its declaration available for cleanup, and preview native removal explicitly. For `ai-cli`:

```bash
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" bootstrap unapply ai-cli --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" bootstrap unapply ai-cli --yes
/usr/bin/python3 "$config_dir/bin/fish-config.py" unapply amp
```

Native unapply does not execute hooks. Remove only the Fish blocks owned by the modules being removed:

| Removed environment | Additional command |
|---|---|
| `ai-cli` | `/usr/bin/python3 "$config_dir/bin/fish-config.py" unapply amp` |
| `terminal` | `/usr/bin/python3 "$config_dir/bin/fish-config.py" unapply activate` |
| Both | Run both commands |

Each helper operation preserves unrelated Fish startup content. Removing `ai-cli` must not remove terminal-owned activation when `terminal` remains selected.

GitHub CLI settings and aliases are application-owned and are not reversed by native unapply; change them through `gh config` and `gh alias` rather than deleting the whole configuration or `hosts.yml`. Cloned repositories, login items, and macOS preferences need separate review. See [Manual actions](manual-actions.md) for preference snapshots and selective reversal.

For native package cleanup, preview candidates explicitly and consult the [mise package lifecycle documentation](https://mise.jdx.dev/bootstrap/packages/) before removing anything:

```bash
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-cli bootstrap packages prune --dry-run
```

Do not apply a broad prune automatically. Keep the Nerd Font while either `terminal` or `development-editors` remains, jq while either AI module remains, and `mas` whenever `foundation` or any App Store module remains. Hook-owned Amp CLI, FFF, and OMP are not owned by native status, upgrade, or prune and can appear as incorrect prune candidates. Remove the six Homebrew exceptions only with their explicit owner-specific commands in [Homebrew exceptions](homebrew-exceptions.md), and review every native candidate individually.

## 6. Troubleshooting

- **Interrupted Command Line Tools or Homebrew prompt:** finish the installer, open a fresh Terminal, and rerun the same stage-0 command. A cancelled prerequisite or empty installer download fails rather than continuing partially.
- **Existing configuration refused:** inspect `git -C "$config_dir" status`, `git -C "$config_dir" rev-parse --show-toplevel`, and its `origin`. Do not delete or overwrite an unrelated checkout; migrate it deliberately.
- **Unexpected project resources:** use `MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir"` for machine setup. `-E` selects environments but does not exclude projects in the caller's directory or above the adopted checkout.
- **Custom XDG paths:** Ghostty and OpenCode use their effective XDG configuration roots; GitHub CLI also honors `GH_CONFIG_DIR`; Fish uses the XDG Fish file. Starship intentionally retains its normal `$HOME/.config/starship.toml` fallback.
- **App Store applications missing:** sign in, claim or purchase the app where required, verify `command -v mas`, and rerun the owning module. Signed-out success does not establish signed-in installation.
- **Amp Desktop does not open:** its package is retained despite a known macOS 27.2 startup failure. Do not bypass signing or security controls; use another supported client until upstream provides a working build.
- **AI accounts or models unavailable:** complete each provider's authentication and opt-in manually. Authenticated OMP routes/instruction loading and interactive Claude rendering are not established by package installation or synthetic tests.
- **Preferences need rollback:** use the exact snapshots captured before applying `system-preferences`. Full-domain import is only for immediate rollback; later changes require selective per-key restoration.
