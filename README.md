# essential-mise

An engine that sets up and maintains an Apple silicon Mac from a **config repository that you own**. The engine is a small bash launcher (`bootstrap.sh`) plus a Rust console (CLI and TUI) over [mise bootstrap](https://mise.jdx.dev/bootstrap.html). It contains no machine configuration: your packages, dotfiles, and module list live in your own repository, described by `#:` header comments in its own config files.

**Status:** pre-1.0. The launcher pins console release `v0.2.0`. The config contract can change in a minor release; the changes are listed in the [release notes](https://github.com/donbeave/essential-mise/releases). Review `bootstrap.sh` and the console release it pins before you run them.

## Requirements

- Apple silicon running macOS **27.2 or newer**, in a native Terminal. The launcher refuses other platforms.
- Internet access and an administrator account for the Command Line Tools and Homebrew prompts.
- A config repository. Accounts, licenses, and App Store sign-in stay manual.

## Quick start

1. Create a config repository from the starter. See [Create your config](docs/create-your-config.md) for the steps and the lint check.
2. On the Mac, run the launcher with your repository:

```bash
/bin/bash -c "$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/donbeave/essential-mise/main/bootstrap.sh)" bootstrap --config OWNER/REPO
```

Replace `OWNER/REPO` with your repository. The launcher checks the platform, downloads the pinned console release, verifies its SHA-256, and starts `essential-mise-console setup`. Setup runs six stages (Command Line Tools, Homebrew, mise, repository access, adoption of your repository as `~/.config/mise`, base modules), then opens the module-selection TUI. Add `--dry-run` after `--config OWNER/REPO` to print the stages without running them. Details and recovery: [First Mac setup](docs/first-mac.md).

## How config repositories work

| Piece | Where |
| --- | --- |
| Engine: launcher, console, VM test harness, docs | this repository |
| Your machine configuration | your config repository |
| Working example config | [`templates/config-starter`](templates/config-starter) |

- The root of your repository becomes the mise config directory.
- `config.toml` carries the repository header (`#: schema`, `#: name`, optional `#: min_console`).
- Each `config.<name>.toml` is a module; the file name is the module name, and its `#: description` line is what the TUI shows. `#: order` sorts the list, and `#: base = true` makes `setup` apply the module automatically.
- Everything else in the files is ordinary mise bootstrap TOML. The console never keeps a module list of its own.

The exact format and rules are in the [config repository contract](docs/config-repo.md).

## Using the console

The console has four steps: **Choose modules**, **Review and confirm**, **Applying**, then **Done** or **Needs attention**. Enter does the main action shown above the footer, Esc goes back, `q` quits, `?` shows the keys for the current screen and the words used. On the last step, `d` lists every pending change and Enter applies again for just those changes.

| Exit code | Meaning |
| --- | --- |
| 0 | Nothing differs, or you left before any work |
| 3 | Changes are still pending when you leave |
| 1 | The check failed, the apply stopped, or you cancelled |

On exit the console prints the outcome and the commands to run next to your terminal. Keys, vocabulary and layout: [console screens](docs/specs/essential-mise-console/screens.md).

## Check a config repository

```bash
essential-mise-console lint --config-dir /path/to/your-config
```

Exit code 0 means the repository satisfies the contract. To get the binary before you bootstrap, see [Create your config](docs/create-your-config.md#3-lint).

## Docs

- [Architecture](docs/architecture.md): engine versus config, data flow, trust model
- [Config repository contract](docs/config-repo.md): layout, the `#:` header format, lint, setup stages, pinning
- [Create your config](docs/create-your-config.md)
- [First Mac setup](docs/first-mac.md)
- [Writing modules](docs/writing-modules.md)
- [Releasing](docs/releasing.md): how console releases and launcher pins are produced
- [VM testing](docs/vm-testing.md) and the [harness reference](testing/vm/README.md)
- [Console specs](docs/specs/essential-mise-console/README.md): design record
- [Agent rules](AGENTS.md)
