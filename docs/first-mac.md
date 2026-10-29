# First Mac setup

## Supported starting point

Use a native Apple Silicon Terminal on macOS **27.2 or newer**. Intel and Rosetta are not supported. On an older factory image, update macOS in System Settings and rerun the command; no OS upgrade is automated.

You need internet access, an admin user, and interactive prompts for Apple Command Line Tools and Homebrew. App Store sign-in is a separate manual step for MAS apps.

## 1. Review and run stage 0

After publication, inspect `bootstrap.sh` in the browser or download it for review. Then use the fail-closed launcher:

```bash
essential_mise_script="$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/donbeave/essential-mise/main/bootstrap.sh)" && test -n "$essential_mise_script" && /bin/bash -c "$essential_mise_script"
```

For a tested release, replace `main` with the reviewed commit SHA. `bash -c`, rather than `curl | bash`, preserves interactive installer input.

Stage 0 performs only:

1. Platform checks.
2. Apple Command Line Tools and Git verification, waiting for Apple’s GUI installation if needed.
3. Official Homebrew installation and guarded `shellenv` initialization. Only its standard line is appended to `~/.zprofile`; the file is not replaced.
4. Official `mise.run` installation of `v2026.10.23`, with exact version verification and `~/.local/bin` added to the current PATH.
5. First-run-only adoption into `$MISE_CONFIG_DIR` (default `~/.config/mise`) from an empty working directory. Existing valid checkouts skip adoption; wrong-origin/conflicting paths fail without modification.
6. Only `mise -E foundation bootstrap`, which adds the native `~/.local/bin` line to `~/.zshenv` and native zsh activation.
7. Fresh login+interactive zsh checks for Git, brew, mise version, and `mise doctor`.

A failed/empty installer download, cancelled CLT installation, invalid `shellenv`, wrong mise version, conflicting config, or failed shell check returns nonzero. No optional environment is selected permanently.

## 2. Standard workstation

Use the same config path everywhere:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
cd "$config_dir"
mise -E foundation,terminal,development,security bootstrap --dry-run
mise -E foundation,terminal,development,security bootstrap
```

If App Store sign-in is unavailable, first apply `foundation,terminal,development`, sign in, then rerun the full standard set. Verify `command -v mas` before expecting MAS installation.

For first-time persistent selection only:

```bash
printf 'env = ["foundation", "terminal", "development", "security"]\n' \
  > "$config_dir/miserc.local.toml"
```

This overwrites an existing selector. To add modules later, review the current file and replace the list while preserving existing entries. Project-level `.miserc*`, `MISE_ENV`, and `-E` can override this global selection.

## 3. Optional environments

Preview first, then apply. For example:

```bash
cd "$config_dir"
mise -E ai-tools,ai-cli bootstrap --dry-run
mise -E ai-tools,ai-cli bootstrap
```

A one-off `-E` replaces persistent selection for that command and does not uninstall other resources. Add an optional module to `miserc.local.toml` to persist it.

For runtime modules, distinguish:

- `mise -E development-java bootstrap`: install once.
- `mise -E development-java exec -- java -version`: invoke once.
- Add `development-java` to `miserc.local.toml`: persist for later shells/commands.

Verify runtime selection in a fresh shell from an unrelated directory after persistence. Homebrew may install transitive Node/OpenJDK dependencies independently of mise’s selected runtime.

## 4. Update an adopted checkout

The adopted config directory is authoritative. The development checkout is not silently synchronized with it.

```bash
cd "$config_dir"
git fetch origin
git diff --stat HEAD..origin/main
git diff HEAD..origin/main
git pull --ff-only
mise bootstrap --dry-run
mise bootstrap
```

Stop on a dirty/conflicting checkout. The final command applies the persistent selection; use explicit `-E` to limit it. Plain `mise bootstrap --update` is a separate package/repository metadata operation.

## 5. Remove an environment

Remove it from `miserc.local.toml`, keep its declaration file for cleanup, then run:

```bash
mise bootstrap unapply ai-cli --dry-run
mise bootstrap unapply ai-cli
```

Unapply covers only supported managed files/directories/services/dotfile edits. Packages, repositories, and preferences need separate cleanup. Package pruning is broad and separately approved, never an automatic next step.
