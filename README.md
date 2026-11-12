# essential-mise

An engine that sets up and maintains an Apple Silicon Mac from a **config repository that you own**. The engine is a small bash launcher plus a Rust console (CLI and TUI) over [mise bootstrap](https://mise.jdx.dev/bootstrap.html). It contains no machine configuration: your packages, dotfiles, and module list live in your own repository, described by `#:` header comments in its own config files.

**Status: PRE-RELEASE.** Review `bootstrap.sh` and the console release it pins before running them.

## Requirements

- Apple Silicon running macOS **27.2 or newer**, in a native Terminal.
- Internet access and an administrator account for Command Line Tools and Homebrew prompts.
- A config repository (see below). Accounts, licenses, and App Store sign-in stay manual.

## Quick start

1. Create your config repository from the starter: see [Create your config](docs/create-your-config.md).
2. On the Mac, run:

```bash
/bin/bash -c "$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/donbeave/essential-mise/main/bootstrap.sh)" bootstrap --config OWNER/REPO
```

Replace `OWNER/REPO` with your repository. The launcher verifies a pinned console release, then the console installs Command Line Tools, Homebrew, and mise, adopts your repository as `~/.config/mise`, applies its base modules, and opens the module-selection TUI. Details and recovery: [First Mac setup](docs/first-mac.md).

## How it fits together

| Piece | Where |
| --- | --- |
| Engine: launcher, console, VM test harness, docs | this repository |
| Your machine configuration | your config repository |
| Working example config | [`templates/config-starter`](templates/config-starter) |

## Check a config repository

```bash
essential-mise-console lint --config-dir /path/to/your-config
```

## Docs

- [Architecture](docs/architecture.md): engine versus config, data flow, trust model
- [Config repository contract](docs/config-repo.md): layout and the `#:` metadata header format
- [Create your config](docs/create-your-config.md)
- [First Mac setup](docs/first-mac.md)
- [Writing modules](docs/writing-modules.md)
- [VM testing](docs/vm-testing.md)
- [Console specs](docs/specs/essential-mise-console/README.md)
