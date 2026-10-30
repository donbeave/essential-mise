# Real Homebrew compatibility exceptions

The repository uses native mise package managers wherever they work. A dependency is moved to real Homebrew only after a concrete incompatibility in **mise 2026.10.23** is identified. Native `[bootstrap.hooks.post-packages]` commands invoke `/opt/homebrew/bin/brew` directly; they are idempotent, appear in dry runs, and do not create a second package owner.

## Approved exception scope and package management

The user explicitly approved five AI compatibility exceptions, the Claude status-line script/wrapper, and a real-Homebrew fallback for demonstrated current-mise failures. Surge is the sixth package exception under that fallback. This is a specification change, not a claim that revision 7's native-only rule is satisfied. Hooks are installation/health-check commands, not declarative package resources. `mise bootstrap status --missing`, native package upgrades, and native prune must not be relied on for these hook-installed packages. Hooks run on every apply, so the required health checks must pass every time.

Every formula and cask hook checks `brew list` first and invokes its install command only when that package is absent. A healthy installed-but-outdated package is left at its current version; routine applies do not invoke Homebrew installation, dependency upgrades, or cleanup for it. Initial installation can resolve and install dependencies normally. The required executable/signature health check always runs. Use the explicit upgrade command only after review; formula upgrades can change dependencies, and cask upgrades can replace application bundles and require fresh privacy consent or login-item review.

The lifecycle fixture proved this with Amp CLI `0.0.1793352886-g7ad87b` while `livecheck` offered `0.0.1793363705-g5c9ae5`: ordinary `ai-cli` bootstrap left the link and complete formula inventory unchanged, and `amp --version` passed. The fixture compares formula state before/after and does not rely on a dry run.

| Package | Install owner when absent | Required health check | Explicit upgrade owner | Manual removal |
|---|---|---|---|---|
| Claude Code | `brew install --cask claude-code@latest` | `/opt/homebrew/bin/claude --version` | `brew upgrade --cask claude-code@latest` | `brew uninstall --cask claude-code@latest` |
| Amp CLI | `brew install ampcode/tap/ampcode` | `/opt/homebrew/bin/amp --version` | `brew upgrade ampcode/tap/ampcode` | `brew uninstall ampcode/tap/ampcode` |
| OMP | `brew install can1357/tap/omp` | `/opt/homebrew/bin/omp --version` | `brew upgrade can1357/tap/omp` | `brew uninstall can1357/tap/omp` |
| FFF MCP | `brew install dmtrKovalenko/fff/fff-mcp` | `/opt/homebrew/bin/fff-mcp --healthcheck` | `brew upgrade dmtrKovalenko/fff/fff-mcp` | `brew uninstall dmtrKovalenko/fff/fff-mcp` |
| Amp Desktop | `brew install --cask ampcode/tap/amp@beta` | Executable exists and `codesign --verify --deep --strict` passes | `brew upgrade --cask ampcode/tap/amp@beta` | `brew uninstall --cask ampcode/tap/amp@beta` |
| Surge | `brew install --cask surge` | `surge-cli`, `surge-dhcpd` are executable and `codesign --verify --deep --strict` passes | `brew upgrade --cask surge` | `brew uninstall --cask surge` |

Deselecting an environment does not remove these packages. Removal is a separate, explicit Homebrew operation; do not use mise prune expecting it to remove only these packages or to preserve unrelated Homebrew installations.

The actual `mise bootstrap packages prune --dry-run` in the validation guest listed `brew:ampcode@0.0.1793318446-g3ee5e2`, `brew:fff-mcp@0.11.0`, and `brew:omp@18.14.0` for removal even though their hooks had installed working tools; it also listed the unrelated fixture `brew:sl@5.02`. **Do not apply a broad native formula prune while these hook-owned packages must remain installed.** Review every candidate and use the explicit Homebrew upgrade/removal commands above instead. No broad prune was executed.

## Claude Code — `claude-code@latest`

**Exact issue:** the `claude-code` cask is unusable in the current installation. Its `/opt/homebrew/bin/claude` link points to `/opt/homebrew/Caskroom/claude-code/2.1.308/bin/claude`, but that path does not exist; invoking `claude` fails with `command not found`. Real Homebrew reports a successful install without repairing the layout. The supported `claude-code@latest` cask provides a working executable and reports `2.1.312 (Claude Code)`.

**Real-Homebrew command:**

```bash
/opt/homebrew/bin/brew install --cask claude-code@latest
```

The hook is declared in `config.ai-cli.toml`, and the broken `brew-cask:claude-code` declaration is removed. This is an upstream cask-variant/layout problem, not a mise-only defect. Existing broken `claude-code` installations were explicitly migrated in the disposable guest with `brew uninstall --cask claude-code` before installing `claude-code@latest`; no ownership transfer is hidden in the hook.

## Amp CLI — `ampcode/tap/ampcode`

**Exact mise issue:** native mise reports a successful source build/install for Amp CLI `0.0.1793304039-g62d85d`, but leaves `/opt/homebrew/Cellar/ampcode/0.0.1793304039-g62d85d/bin/amp` with mode `644`. Running `amp --version` fails with `permission denied`.

**Real-Homebrew command:**

```bash
/opt/homebrew/bin/brew install ampcode/tap/ampcode
```

Real Homebrew installs the same binary with mode `555`, and `amp --version` succeeds. The hook is declared in `config.ai-cli.toml`. The native formula declaration and the Ruby prerequisite used only for its evaluation were removed. Existing broken native installations were repaired in the disposable guest with `brew reinstall ampcode/tap/ampcode`; fresh machines need only the hook.

## OMP — `can1357/tap/omp`

**Exact mise issue:** native source-formula installation fails with:

```text
Error: formula uses `with_env (install-time helper)`, which mise's source-build shim does not support
```

The formula downloads a prebuilt OMP binary but wraps completion generation in `with_env(HOME: buildpath)`. Native mise cannot execute that helper. The former `brew:can1357/tap/omp` declaration and alternative native HTTP declaration were removed.

**Real-Homebrew command:**

```bash
/opt/homebrew/bin/brew install can1357/tap/omp
```

Homebrew evaluates the formula itself, including `with_env` and completions. The hook is declared in `config.ai-cli.toml`; `brew install` is safe to rerun when OMP is already installed. OMP model/instruction files remain native mise dotfile merges.

## FFF MCP — `dmtrKovalenko/fff/fff-mcp`

**Exact mise issue:** native mise 2026.10.23 resolves, downloads, and reports installing `fff-mcp` 0.11.0, but leaves `/opt/homebrew/Cellar/fff-mcp/0.11.0/bin/fff-mcp` with mode `644`. Running it fails with `permission denied`; the formula's own `fff-mcp --healthcheck` cannot run. The formula uses `bin.install`, which must create an executable.

**Real-Homebrew command:**

```bash
/opt/homebrew/bin/brew install dmtrKovalenko/fff/fff-mcp
```

Real Homebrew installs the binary with mode `555`, and its health check reports `All checks passed`. The hook is declared in `config.ai-tools.toml`. The native tap declaration and the now-unneeded `[tools]` Ruby prerequisite were removed from that module. Existing broken native installations were repaired in the disposable guest with an explicit tap plus `brew reinstall dmtrKovalenko/fff/fff-mcp`; fresh machines need only the hook above.

## Amp Desktop — `ampcode/tap/amp@beta`

**Exact mise issue:** the cask uses `version.csv.second` to construct its DMG URL. Native mise's cask evaluation fails with:

```text
undefined method 'second' for an instance of Array
```

Real Homebrew supports this Ruby DSL. The former native `macos-app:amp` declaration was removed, so mise and Homebrew do not both own Amp.

**Real-Homebrew command:**

```bash
/opt/homebrew/bin/brew install --cask ampcode/tap/amp@beta
```

The validated cask is version `1.0,998`, using the official `Amp-1.0-998.dmg` and SHA-256 `5fc0e7e0b1f1644ea0d234aae70832caae70677ebde79eb645e2a623314dad6b`. The hook in `config.ai-desktop.toml` first refuses an existing `/Applications/Amp.app` that is not registered to this Homebrew cask. Migrate such an installation manually before applying the module; there is no automatic adoption or ownership transfer.

## Surge — `surge`

**Exact mise issue:** the cask declares a binary at `$APPDIR/Surge.app/Contents/Applications/Surge Dashboard.app`. The nested app is present in the downloaded bundle, but native mise 2026.10.23 fails with:

```text
brew-cask: binary artifact '$APPDIR/Surge.app/Contents/Applications/Surge Dashboard.app' was not found
```

The native declaration is removed. The `network` module's real-Homebrew hook refuses an existing unmanaged `Surge.app` or `Surge Dashboard.app` rather than transferring ownership. During validation, the known incomplete mise app was moved into the evidence directory before running Homebrew; no existing user installation was adopted.

```bash
/opt/homebrew/bin/brew install --cask surge
```

Real Homebrew installed Surge `6.10.0,12500,79b87792e2bd345e064c2d473790b404`, linked `Surge Dashboard.app`, `surge-cli`, and `surge-dhcpd`, and passed executable and signature health checks. Homebrew owns upgrades and explicit removal. Proxy endpoints, secrets, privileged helpers, and application configuration remain manual.

## Related decisions that are not Homebrew exceptions

- No separate Ruby prerequisite remains. All third-party formulae with demonstrated native-mise failures use real Homebrew, which evaluates their Ruby definitions itself.
- Amp CLI uses real Homebrew because its native installation reported success but left a mode-644, non-executable binary.
- FFF uses real Homebrew because the native package's mode-644 executable failure was reproduced and repaired successfully with Homebrew. The earlier anonymous GitHub API `403` rate limit was transient and was not the reason for changing owners.
- Every additional exception must record the exact mise error and be verified on disposable macOS 27.2 before it is added here.

## Official direct-download fallback that is not a Homebrew exception

**Grok Bot:** both native mise and real Homebrew fail to download `https://downloads.cursor.com/sand/stable/darwin-arm64/0.78.1/Grok_Bot_0.78.1.dmg` with HTTP 403. The [official Grok Bot download page](https://cursor.com/download/bot) instead links `https://downloads.cursor.com/grokbot/stable/darwin-arm64/0.78.1/Grok_Bot_0.78.1.dmg`. The cask's `/sand/` path is outdated; this is not a mise-specific incompatibility and real Homebrew cannot repair it.

The current official DMG downloads successfully, has SHA-256 `0e575dfd2f638808f0487f88dec6bf53a871cac2939de7fe454981f927f9f0d9` (identical to the cask's expected artifact), and passes Developer ID/notarization verification. `config.ai-desktop.toml` therefore uses native `macos-app:grok-bot` with this exact official URL, checksum, and `Grok Bot.app` artifact, without adoption. No alternate source or custom installer is used.

On the clean `admin`/`admin` disposable guest, real Homebrew installed Amp Desktop `1.0,998`, Amp CLI `0.0.1793318446-g3ee5e2`, OMP `18.14.0`, and FFF `0.11.0`. Amp and FFF executable health checks passed, and the full AI scenario plus second-run hooks converged. Full AI Desktop also passed with the official Grok DMG. A separate unmanaged `/Applications/Amp.app` triggered the hook's refusal without ownership transfer. Amp's package installation is verified; independent clean guests reproduced its `_dyld_start` GUI stall. Scoped v1 explicitly retains the package with that known nonfunctional-GUI limitation; GUI setup is MANUAL/DROP on macOS 27.2 and is documented in `docs/validation.md`.
