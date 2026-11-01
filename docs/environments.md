# Environments

Each `config.NAME.toml` is selected explicitly with `-E NAME` or persisted in ignored `miserc.local.toml`. No module selects another module implicitly. Set `config_dir` as shown in the README, then preview with `MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E NAME bootstrap --dry-run` before applying.

The TOML files are the package and setting source of truth.

## Standard selection

Recommended persistent list:

```toml
env = ["foundation", "terminal", "development", "security"]
```

| Module | Purpose and contents | Prerequisites and boundaries |
|---|---|---|
| `foundation` | `mas` CLI; `~/.local/bin` in `~/.zshenv`; native zsh activation | Installs the default App Store CLI; no optional configuration |
| `terminal` | Stable Ghostty, JetBrainsMono Nerd Font, Starship, bat, watch, just, xh, wget, Bash, Nushell, Fish, git-standup, msedit, buf, gogcli; pinned Oh My Zsh and zsh-autosuggestions | Starship is the only prompt renderer; Oh My Zsh self-update is disabled; Ghostty follows `XDG_CONFIG_HOME` through native variants; Fish activation uses an additive XDG-aware block |
| `development` | GitHub CLI and Git LFS | Nonsecret GitHub CLI settings and the `co` alias use `gh` itself, preserving unrelated keys and honoring `GH_CONFIG_DIR`/custom XDG. Git tuning uses the manual `git config` commands in [Manual actions](manual-actions.md); identity, `git lfs install`, and `gh auth setup-git` are manual |
| `security` | 1Password, 1Password CLI, and 1Password for Safari (`mas:1569813296`) | App Store sign-in, account unlock, SSH agent, extension enablement, and CLI auth are manual |

## Optional development stacks

| Module | Contents | Prerequisites and boundaries |
|---|---|---|
| `js` | Node `26.9.0`, Bun `1.4.4` | Install once, invoke once with `-E ... exec`, or persist in `miserc.local.toml` |
| `development-rust` | Rust `1.99.1` with clippy, rustfmt, rust-analyzer, rust-src, `llvm-tools`, and native `mr_boxington`; mr-boxington `1.24.1`; cargo-nextest, cargo-llvm-cov, cargo-deny, and bacon | Uses native Cargo command-wrapper behavior; `gh auth` may be required for release lookup |
| `development-java` | Oracle GraalVM `25.0.4.1.1`, Gradle, Maven, VisualVM | Homebrew OpenJDK may also be installed; verify GraalVM, `mvn`, and `gradle` in a fresh shell |
| `development-apps` | JetBrains Toolbox, Yaak, TablePlus, DevHub, Patterns | Toolbox IDEs/settings and MAS sign-in are manual |
| `development-containers` | OrbStack | VM/runtime/credential onboarding is manual |
| `development-editors` | Sublime Text, VS Code, stable Zed, JetBrainsMono Nerd Font | Zed merge uses JSONC; AI-provider keys are excluded; the font is declared here for independent selection |
| `development-apple` | Xcode through MAS | Full Xcode onboarding and licenses are manual |
| `kubernetes` | kubectl, kubectx, Helm | Cluster credentials and setup are manual |
| `infrastructure` | OpenTofu, Ansible, linode-cli, gomplate | CLI only; no provisioning projects or state |

## Optional application groups

| Module | Contents | Manual boundaries |
|---|---|---|
| `utilities` | CleanShot, Bartender, Homebrew App, Keka | Additive login items use `/Applications/CleanShot X.app` and `/Applications/Bartender 7.app` |
| `messengers` | Telegram, WhatsApp, Slack, WeChat, Zoom | Sign-in and notification/privacy setup |
| `browsers` | Google Chrome, Google Chrome Dev, Firefox Developer Edition | Browser profiles, extensions, and sign-in |
| `privacy` | Tor Browser, Little Snitch | System extension and privacy approvals |
| `productivity` | Notion, Grammarly Desktop/Safari, Speechify, Superwhisper, Raindrop.io, Reeder | Superwhisper configuration is fully manual; no mode files are deployed |
| `office` | Word, Excel, PDF Expert, Delimited, Create Booklet 2 | Licenses, accounts, and app setup |
| `design` | Sketch, Pixelmator Pro | License and Sketch Library registration |
| `media` | VLC, Transmission, HandBrake, Plex Media Server, Blackmagic RAW, Shazam | Media server setup and accounts |
| `network` | Surge, Cloudflare WARP, Tunnelblick, WhyFi | Surge uses a real-Homebrew hook because mise cannot expand its `$APPDIR` binary artifact. Proxy/VPN endpoints, secrets, extensions, and privileged setup are manual |
| `admin` | Transmit, Apple Remote Desktop, Disk Aid, iStatistica Pro, Apple Configurator (`mas:1037126344`) | Device enrollment, MDM, and administrator approval are manual |

`foundation` installs `brew:mas` during stage 0, before any optional App Store module. Every MAS-owning module also declares it so standalone module selection remains independent. App Store sign-in, purchases/claims, actual signed-in installation, and reruns are manual and unverified. The standard workstation is not described as fully tested.

## AI support and applications

| Module | Contents | Independence and boundaries |
|---|---|---|
| `ai-tools` | ripgrep (`rg`), fd, fzf, ast-grep (`sg`), semgrep, jq, yq, dasel, sd, git-delta, eza, zoxide, tree, tokei, hyperfine, shellcheck, shfmt, universal-ctags, rtk, agent-browser; fff-mcp through real Homebrew | Shared utilities only; no agent-owned settings. Browser runtime initialization is manual. FFF uses real Homebrew because mise leaves its raw binary non-executable |
| `ai-cli` | Claude Code, Codex, Cursor CLI, Antigravity CLI, Kimi Code, OpenCode, Grok Build, Amp CLI, OMP, jq, Claude status line | No desktop packages or references to unselected RTK/FFF/Caveman. Amp CLI and OMP use real-Homebrew hooks for documented incompatibilities. Includes agent instructions, Claude Code's `CLAUDE.md`, subagents, `repo-context` skill, project notes, and pstack model sheet, Amp completions, and the zsh status-line wrapper. Copy any edits to the live Claude files back with `mise dot add --changed` |
| `ai-desktop` | Claude, ChatGPT, Cursor, Antigravity, Kimi, OpenCode Desktop, Grok Bot, ZCode, CodexBar, Amp Desktop via real Homebrew | No CLI packages or settings. Amp Desktop refuses existing unmanaged apps. Its package is retained despite a known macOS 27.2 `_dyld_start` stall; a working GUI is not claimed |

Use these commands for the complete terminal coding-agent setup:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap status --missing
```

`ai-tools` is optional and is never implicitly selected by either application module. OpenCode global instructions follow `XDG_CONFIG_HOME`. `OPENCODE_CONFIG_DIR` is a user-owned override: when set, OpenCode uses it instead and essential-mise does not duplicate instructions there.

## `system-preferences`

Optional, user-scope macOS defaults for Dock, keyboard, Finder, locale, date/time, Siri, screensaver, verification-code deletion, TextInputMenu, and storage. `AppleFirstWeekday` and `AppleICUDateFormatStrings` use nested entries to preserve unrelated dictionary members.

Review the dry run carefully because Trash removal, verification-code deletion, storage optimization, locale, and Siri changes have real side effects. See [Manual actions](manual-actions.md) for snapshots and reversal. Symbolic hotkeys are not automated.

## `workspaces`

No active workspace module or placeholder is shipped. When real repository details exist, create ignored `config.workspaces.local.toml` directly with native `[bootstrap.repos]` and directories, then select `-E workspaces`. Do not commit private repository URLs or synthetic placeholders.
