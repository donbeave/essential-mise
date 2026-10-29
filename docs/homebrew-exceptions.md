# Real Homebrew compatibility exceptions

The repository uses native mise package managers wherever they work. A dependency is moved to real Homebrew only after a concrete incompatibility in **mise 2026.10.23** is identified. Native `[bootstrap.hooks.post-packages]` commands invoke `/opt/homebrew/bin/brew` directly; they are idempotent, appear in dry runs, and do not create a second package owner.

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

The current cask is version `1.0,996`, using the official `Amp-1.0-996.dmg` and SHA-256 `b589a47dc32a99e045d2aa92236e55317eccda64e7ee76089751012beecb3c7a`. The hook in `config.ai-desktop.toml` first refuses an existing `/Applications/Amp.app` that is not registered to this Homebrew cask. Migrate such an installation manually before applying the module; there is no automatic adoption or ownership transfer.

## Related decisions that are not Homebrew exceptions

- No separate Ruby prerequisite remains. All third-party formulae with demonstrated native-mise failures use real Homebrew, which evaluates their Ruby definitions itself.
- Amp CLI uses real Homebrew because its native installation reported success but left a mode-644, non-executable binary.
- FFF uses real Homebrew because the native package's mode-644 executable failure was reproduced and repaired successfully with Homebrew. The earlier anonymous GitHub API `403` rate limit was transient and was not the reason for changing owners.
- Every additional exception must record the exact mise error and be verified on disposable macOS 27.2 before it is added here.

## Official direct-download fallback that is not a Homebrew exception

**Grok Bot:** both native mise and real Homebrew fail to download `https://downloads.cursor.com/sand/stable/darwin-arm64/0.78.1/Grok_Bot_0.78.1.dmg` with HTTP 403. The [official Grok Bot download page](https://cursor.com/download/bot) instead links `https://downloads.cursor.com/grokbot/stable/darwin-arm64/0.78.1/Grok_Bot_0.78.1.dmg`. The cask's `/sand/` path is outdated; this is not a mise-specific incompatibility and real Homebrew cannot repair it.

The current official DMG downloads successfully, has SHA-256 `0e575dfd2f638808f0487f88dec6bf53a871cac2939de7fe454981f927f9f0d9` (identical to the cask's expected artifact), and passes Developer ID/notarization verification. `config.ai-desktop.toml` therefore uses native `macos-app:grok-bot` with this exact official URL, checksum, and `Grok Bot.app` artifact, without adoption. No alternate source or custom installer is used.

On the clean `admin`/`admin` disposable guest, real Homebrew installed Amp Desktop `1.0,996`, Amp CLI `0.0.1793304039-g62d85d`, OMP `18.14.0`, and FFF `0.11.0`. Amp and FFF executable health checks passed, and the full AI scenario plus second-run hooks converged. Full AI Desktop also passed with the official Grok DMG. The separate existing-Amp refusal test is recorded in `docs/validation.md`.
