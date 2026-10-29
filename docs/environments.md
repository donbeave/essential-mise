# Environments

Each `config.NAME.toml` is selected explicitly with `-E NAME` or persisted in ignored `miserc.local.toml`. No module selects another module implicitly. Preview with `mise -E NAME bootstrap --dry-run` before applying.

The TOML files are the exact package/setting source of truth. `docs/migration-decisions.md` maps every declaration to its validation and migration disposition.

## Standard selection

Recommended persistent list:

```toml
env = ["foundation", "terminal", "development", "security"]
```

| Module | Purpose and contents | Prerequisites and boundaries |
|---|---|---|
| `foundation` | `~/.local/bin` in `~/.zshenv`; native zsh activation | No packages or optional configuration |
| `terminal` | Stable Ghostty, JetBrainsMono Nerd Font, Starship, bat, watch, just, xh, wget, Bash, Nushell, Fish, git-standup, msedit, buf, gogcli; pinned Oh My Zsh and zsh-autosuggestions | Starship is the only prompt renderer; Oh My Zsh self-update disabled; Fish uses native `auto` activation, not a copied config file |
| `development` | GitHub CLI and Git LFS | Nonidentity Git settings and nonsecret GitHub CLI config only; real identity, `git lfs install`, and `gh auth setup-git` are manual |
| `security` | 1Password, 1Password CLI, and 1Password for Safari (`mas:1569813296`) | App Store sign-in, account unlock, SSH agent, extension enablement, and CLI auth are manual |

## Optional development stacks

| Module | Contents | Validation and boundaries |
|---|---|---|
| `js` | Node `26.9.0`, Bun `1.4.4` | Install once, invoke once with `-E ... exec`, or persist in `miserc.local.toml` |
| `development-rust` | Rust `1.99.1` with `llvm-tools` and native `mr_boxington`; mr-boxington `1.24.1` | No legacy Cargo wrapper or release-age exception; `gh auth` may be required for release lookup |
| `development-java` | Oracle GraalVM `25.0.4.1.1`, Gradle, Maven, VisualVM | Homebrew OpenJDK may also be installed; verify GraalVM, `mvn`, and `gradle` in a fresh shell |
| `development-apps` | JetBrains Toolbox, Yaak, TablePlus, DevHub, Patterns | Toolbox IDEs/settings and MAS sign-in are manual |
| `development-containers` | OrbStack | VM/runtime/credential onboarding is manual |
| `development-editors` | Sublime Text, VS Code, stable Zed, JetBrainsMono Nerd Font | Zed merge uses JSONC; AI-provider keys are excluded; font is also declared here for independent selection |
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
| `network` | Surge, Cloudflare WARP, Tunnelblick, WhyFi | Proxy/VPN endpoints, secrets, extensions, and privileged setup are manual |
| `admin` | Transmit, Apple Remote Desktop, Disk Aid, iStatistica Pro, Apple Configurator (`mas:1037126344`) | Apple Configurator is NEW; device enrollment, MDM, and admin approval are manual |

Every MAS-owning module also declares `brew:mas` so it is independent. App Store sign-in, purchases/claims, and a possible second apply are manual.

## AI support and applications

| Module | Contents | Independence |
|---|---|---|
| `ai-tools` | ripgrep (`rg`), fd, fzf, ast-grep (`sg`), semgrep, jq, yq, dasel, sd, git-delta, eza, zoxide, tree, tokei, hyperfine, shellcheck, shfmt, universal-ctags, rtk, agent-browser; conditional fff-mcp | Shared utilities only; no agent-owned settings. Browser runtime initialization is manual. FFF is kept only if native mise support is proven |
| `ai-cli` | Claude Code, Codex, Cursor CLI, Antigravity CLI, Kimi Code, OpenCode, Grok Build, Amp CLI, OMP | No desktop packages or references to unselected RTK/FFF/Caveman. Amp CLI is native. OMP uses a documented real-Homebrew hook because mise 2026.10.23 cannot execute the formula's `with_env` helper. Includes self-contained agent instructions and Amp completions |
| `ai-desktop` | Claude, ChatGPT, Cursor, Antigravity, Kimi, OpenCode Desktop, Grok Bot, ZCode, CodexBar, Amp Desktop via real Homebrew | No CLI packages or settings. Amp Desktop uses `ampcode/tap/amp@beta` because mise cannot evaluate the cask's `Array#second` DSL; existing unmanaged apps are refused. See [Homebrew exceptions](homebrew-exceptions.md) |

Use `mise -E ai-tools,ai-cli bootstrap` for the complete terminal coding-agent setup. `ai-tools` is optional and is never implicitly selected by either application module. If a first-time preview reports that Ruby 3+ is required, run `mise -E ai-tools,ai-cli install ruby` before the preview, then apply and run `mise -E ai-tools,ai-cli bootstrap status --missing`. Both modules declare Ruby 4.0.9 under `[tools]`, so mise has its own activated runtime for tap evaluation. This tool-only prerequisite is not a custom installer and does not install other AI packages.

## `system-preferences`

Optional, user-scope macOS defaults for Dock, keyboard, Finder, locale, date/time, Siri, screensaver, verification-code deletion, TextInputMenu, and storage. `AppleFirstWeekday` and `AppleICUDateFormatStrings` use nested entries to preserve unrelated dictionary members.

This module requires explicit review because Trash removal, verification-code deletion, storage optimization, locale, and Siri changes have real side effects. See [manual-actions.md](manual-actions.md) for reversal behavior. Symbolic hotkeys are not automated.

## `workspaces`

No active workspace module or placeholder is shipped. When real repository details exist, create ignored `config.workspaces.local.toml` directly with native `[bootstrap.repos]` and directories, then select `-E workspaces`. Fake URLs and old retry/secondary-remote orchestration are not used.
