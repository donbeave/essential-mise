# Real Homebrew compatibility exceptions

The repository uses native mise package managers wherever they work. A dependency is moved to real Homebrew only after a concrete incompatibility in **mise 2026.10.23** is identified. Native `[bootstrap.hooks.post-packages]` commands invoke `/opt/homebrew/bin/brew` directly; they are idempotent, appear in dry runs, and do not create a second package owner.

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

- Third-party tap evaluation uses pinned `ruby = "4.0.9"` under `[tools]`. A `brew:ruby` package installs `/opt/homebrew/bin/ruby` but does not provide mise's own activated formula-evaluation runtime.
- Amp CLI remains native `brew:ampcode/tap/ampcode`; its actual native source build succeeds.
- FFF remains native unless a concrete mise incompatibility is proven. An anonymous GitHub API `403` rate limit is an external transient condition, not a reason to change package owners.
- Every additional exception must record the exact mise error and be verified on disposable macOS 27.2 before it is added here.

Clean `admin`/`admin` VM verification of these real-Homebrew hooks is pending; earlier native-package results do not establish this revised path.
