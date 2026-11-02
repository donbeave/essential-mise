# Real Homebrew compatibility exceptions

Native mise package resources are the default. These six packages use `/opt/homebrew/bin/brew` directly because a concrete incompatibility was reproduced with mise **2026.10.23**. The hooks install or upgrade their named packages and run health checks; they are not declarative package resources.

Every hook checks the expected Homebrew owner before installing, and upgrades the named package on subsequent applies. Cask upgrades use `--greedy`, including casks that update themselves or declare a `latest` version. Native `status --missing`, upgrade, and prune do not manage these packages.

## Ownership and lifecycle

| Package and owning module | Install when absent | Health check on every apply | Explicit upgrade | Explicit removal |
|---|---|---|---|---|
| Claude Code (`ai-cli`) | `brew install --cask claude-code@latest` | `/opt/homebrew/bin/claude --version` | `brew upgrade --cask --greedy claude-code@latest` | `brew uninstall --cask claude-code@latest` |
| Amp CLI (`ai-cli`) | `brew install ampcode/tap/ampcode` | `/opt/homebrew/bin/amp --version` | `brew upgrade ampcode/tap/ampcode` | `brew uninstall ampcode/tap/ampcode` |
| OMP (`ai-cli`) | `brew install can1357/tap/omp` | `/opt/homebrew/bin/omp --version` | `brew upgrade can1357/tap/omp` | `brew uninstall can1357/tap/omp` |
| FFF MCP (`ai-tools`) | `brew install dmtrKovalenko/fff/fff-mcp` | `/opt/homebrew/bin/fff-mcp --healthcheck` | `brew upgrade dmtrKovalenko/fff/fff-mcp` | `brew uninstall dmtrKovalenko/fff/fff-mcp` |
| Amp Desktop (`ai-desktop`) | `brew install --cask ampcode/tap/amp@beta` | Executable exists and `codesign --verify --deep --strict` succeeds | `brew upgrade --cask --greedy ampcode/tap/amp@beta` | `brew uninstall --cask ampcode/tap/amp@beta` |
| Surge (`network`) | `brew install --cask surge` | `surge-cli` and `surge-dhcpd` are executable; application signature verification succeeds | `brew upgrade --cask --greedy surge` | `brew uninstall --cask surge` |

Deselecting a module does not remove its hook-owned packages. Removal is always a separate Homebrew operation. Formula upgrades can change dependencies; cask upgrades can replace application bundles and require renewed privacy consent or login-item review.

A native formula-prune dry run can offer working hook-installed Amp CLI, FFF, and OMP for removal, along with unrelated Homebrew packages. **Do not apply a broad native formula prune while those tools must remain installed.** Review every candidate and use the explicit commands above.

## Exact compatibility reasons

### Claude Code — `claude-code@latest`

The ordinary `claude-code` cask produced a `/opt/homebrew/bin/claude` link to a missing `bin/claude`, so invocation failed with `command not found`. The supported `claude-code@latest` cask supplies a working executable. The broken native declaration was removed; no automatic migration or second owner is introduced.

### Amp CLI — `ampcode/tap/ampcode`

Native mise reported a successful build/install but left the Cellar `bin/amp` with mode `644`; `amp --version` failed with `permission denied`. Real Homebrew installs an executable binary and passes the health check.

### OMP — `can1357/tap/omp`

Native source-formula evaluation fails with:

```text
Error: formula uses `with_env (install-time helper)`, which mise's source-build shim does not support
```

Homebrew evaluates the formula and its completion-generation helper normally. OMP instruction/model files remain native mise dotfile merges.

### FFF MCP — `dmtrKovalenko/fff/fff-mcp`

Native mise installed the raw `fff-mcp` binary with mode `644`, so its own `--healthcheck` could not execute. Real Homebrew installs it executable and the health check passes. No separate Ruby prerequisite is needed.

### Amp Desktop — `ampcode/tap/amp@beta`

Native cask evaluation fails while constructing the DMG URL:

```text
undefined method 'second' for an instance of Array
```

Real Homebrew supports this cask DSL. The hook refuses an existing `/Applications/Amp.app` not owned by this cask; migrate that application manually before applying `ai-desktop`. Package provisioning is retained despite a known macOS 27.2 `_dyld_start` GUI stall. A working GUI is not claimed, and no security bypass is permitted.

### Surge — `surge`

The cask declares a nested binary using a literal `$APPDIR` source. Native mise fails with:

```text
brew-cask: binary artifact '$APPDIR/Surge.app/Contents/Applications/Surge Dashboard.app' was not found
```

Real Homebrew installs and links the application and command-line tools. The `network` hook refuses unmanaged existing Surge applications rather than transferring ownership. Proxy/VPN endpoints, secrets, privileged helpers, and application configuration remain manual.

## Official direct download that is not an exception

Grok Bot's outdated cask URL uses `/sand/` and returns HTTP 403 through both mise and Homebrew. The [official download page](https://cursor.com/download/bot) instead provides:

```text
https://downloads.cursor.com/grokbot/stable/darwin-arm64/0.78.1/Grok_Bot_0.78.1.dmg
```

`config.ai-desktop.toml` uses that official DMG as a native `macos-app:grok-bot` resource with SHA-256 `0e575dfd2f638808f0487f88dec6bf53a871cac2939de7fe454981f927f9f0d9` and artifact `Grok Bot.app`. This is an official-source correction, not a custom installer or Homebrew ownership exception.

Any additional exception requires a reproduced current-mise incompatibility and documented installation owner, health check, upgrade, removal, and native-prune behavior.
