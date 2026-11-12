# Create your config

A config repository holds everything specific to your Mac. Start from the starter in this repository.

## 1. Copy the starter

```bash
cp -R /path/to/essential-mise/templates/config-starter ~/my-config
cd ~/my-config
git init -b main
```

The starter contains a `config.toml` with a repository header, shared settings, a `foundation` base module, an example `terminal` module with one dotfile, and a `PIN-EXCEPTIONS.md` file. See [the contract](config-repo.md) for every field.

## 2. Edit the headers

In `config.toml`, set `name` in the `#:` header; add `#: min_console = "0.2.0"` if your config needs that console or newer. Keep `#: base = true` in `config.foundation.toml`. Start each `config.<name>.toml` you create with a `#: description = "..."` line (and `#: order = <number>` to place it in the list). Write modules as described in [Writing modules](writing-modules.md).

## 3. Lint

Lint needs the console binary. Either use a binary the launcher already cached, or download the release asset and verify it:

```bash
# After a first bootstrap run on this Mac:
emc="$(ls "${XDG_CACHE_HOME:-$HOME/.cache}"/essential-mise/essential-mise-console-v* | grep -v '\.tar\.gz$' | tail -n 1)"

# Or download and verify it (Apple silicon):
gh release download v0.2.0 --repo donbeave/essential-mise \
  --pattern 'essential-mise-console-aarch64-apple-darwin.tar.gz' --dir /tmp/emc
gh attestation verify /tmp/emc/essential-mise-console-aarch64-apple-darwin.tar.gz --repo donbeave/essential-mise
tar -xzf /tmp/emc/essential-mise-console-aarch64-apple-darwin.tar.gz -C /tmp/emc
emc=/tmp/emc/essential-mise-console
```

Then:

```bash
"$emc" lint --config-dir .
```

Fix every reported problem. Exit code 0 prints `lint: . is valid` and means the repository satisfies the contract. Contributors to the engine can run `cargo run -q -- lint --config-dir <dir>` in `apps/essential-mise-console` instead.

## 4. Publish it privately

```bash
git add -A && git commit -m "feat: initial machine configuration"
gh repo create OWNER/REPO --private --source . --push
```

## 5. Run bootstrap on the Mac

```bash
/bin/bash -c "$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/donbeave/essential-mise/main/bootstrap.sh)" bootstrap --config OWNER/REPO
```

Because the repository is private, stage 4 (repository access) may ask you to sign in with the GitHub CLI; see [First Mac setup](first-mac.md).

## Keep secrets out

`.gitignore` must cover `miserc.local.toml` (lint checks this). Do not commit tokens, keys, or account identifiers you consider private; reference them from a secret manager.

## Pin versions

Pin every version that can be pinned and document the rest; see [Pinning](config-repo.md#pinning).
