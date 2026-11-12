# Create your config

A config repository holds everything specific to your Mac. Start from the starter in this repository.

## 1. Copy the starter

```bash
cp -R /path/to/essential-mise/templates/config-starter ~/my-config
cd ~/my-config
git init -b main
```

The starter contains a `config.toml` with a repository header, shared settings, a `foundation` base module, and an example `terminal` module with one dotfile. See [the contract](config-repo.md) for every field.

## 2. Edit the headers

In `config.toml`, set `name` in the `#:` header. Keep `#: base = true` in `config.foundation.toml`. Start each `config.<name>.toml` you create with a `#: description = "..."` line (and `#: order = <number>` to place it in the list). Write modules as described in [Writing modules](writing-modules.md).

## 3. Lint

```bash
essential-mise-console lint --config-dir .
```

Fix every reported problem. Exit code 0 means the repository satisfies the contract.

## 4. Publish it privately

```bash
git add -A && git commit -m "feat: initial machine configuration"
gh repo create OWNER/REPO --private --source . --push
```

## 5. Run bootstrap on the Mac

```bash
/bin/bash -c "$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/donbeave/essential-mise/main/bootstrap.sh)" bootstrap --config OWNER/REPO
```

Because the repository is private, stage 4 may ask you to sign in with the GitHub CLI; see [First Mac setup](first-mac.md).

## Keep secrets out

`.gitignore` must cover `miserc.local.toml` (lint checks this). Do not commit tokens, keys, or account identifiers you consider private; reference them from a secret manager.
