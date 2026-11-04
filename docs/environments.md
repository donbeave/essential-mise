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
| `terminal` | Stable Ghostty, JetBrainsMono Nerd Font, Starship, bat, watch, just, xh, wget, Bash, Nushell, Fish, git-standup, msedit, buf, gogcli; Oh My Zsh and zsh-autosuggestions | Starship is the only prompt renderer; Oh My Zsh self-update is disabled; Ghostty follows `XDG_CONFIG_HOME` through native variants; Fish activation uses an additive XDG-aware block |
| `development` | GitHub CLI and Git LFS | Configures Git pulls to use merge commits; other Git tuning uses the manual commands in [Manual actions](manual-actions.md). GitHub CLI settings and the `co` and `prd` aliases use `gh` itself, preserving unrelated keys and honoring `GH_CONFIG_DIR`/custom XDG. A `~/.zshenv` `gh` function makes `gh pr create` open a draft ([Draft pull requests by default](gh-draft.md)). Identity, `git lfs install`, and `gh auth setup-git` are manual |
| `security` | 1Password, 1Password CLI, and 1Password for Safari (`mas:1569813296`) | App Store sign-in, account unlock, SSH agent, extension enablement, and CLI auth are manual |

## Optional development stacks

| Module | Contents | Prerequisites and boundaries |
|---|---|---|
| `js` | Latest Node and Bun | Install once, invoke once with `-E ... exec`, or persist in `miserc.local.toml` |
| `development-rust` | Latest Rust with clippy, rustfmt, rust-analyzer, rust-src, `llvm-tools`, and native `mr_boxington`; latest mr-boxington, cargo-nextest, cargo-llvm-cov, cargo-deny, and bacon | Uses native Cargo command-wrapper behavior; `gh auth` may be required for release lookup |
| `development-java` | Latest Oracle GraalVM, Gradle, Maven, VisualVM | Homebrew OpenJDK may also be installed; verify GraalVM, `mvn`, and `gradle` in a fresh shell |
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
| `ai-tools` | ripgrep (`rg`), fd, fzf, ast-grep (`sg`), semgrep, jq, yq, dasel, sd, git-delta, eza, zoxide, tree, tokei, hyperfine, shellcheck, shfmt, universal-ctags, rtk, agent-browser; fff-mcp through real Homebrew | Shared utilities only. Its `post-dotfiles` hook `bin/rtk-agents.sh` enables RTK for the supported installed agents ([RTK](#rtk)); it owns no other agent settings. Browser runtime initialization is manual. FFF uses real Homebrew because mise leaves its raw binary non-executable |
| `ai-cli` | Claude Code, Codex, Cursor CLI, Antigravity CLI, Kimi Code, MiMo Code, OpenCode, Grok Build, Muse Code, Amp CLI, OMP, jq, Claude status line, pstack and i-have-adhd plugin integrations where available | No desktop packages or FFF/Caveman references. The only RTK references are the instruction lines described under [RTK](#rtk). Amp CLI, OMP, and Muse Code use real-Homebrew hooks for documented incompatibilities. Includes agent instructions, Claude Code's `CLAUDE.md`, the `tailrocks-*` subagents (with Grok copies), the `repo-context` and stack skills, project notes, and pstack model sheet; merged `settings.json` keys for the default model (Sonnet), per-model effort, subagent model and spawn depth, and permission allow/deny lists (these arrays replace the live ones); Amp completions; the zsh status-line wrapper; and the zsh aliases listed in [Yolo aliases](yolo.md). Copy any edits to the live Claude files back with `mise dot add --changed`, but do not copy back `model` or `modelSettings` after a temporary `/model` or `/effort` switch |
| `ai-desktop` | Claude, ChatGPT, Cursor, Antigravity, Kimi, OpenCode Desktop, Grok Bot, ZCode, CodexBar, Amp Desktop via real Homebrew | No CLI packages or settings. Amp Desktop refuses existing unmanaged apps. Its package is retained despite a known macOS 27.2 `_dyld_start` stall; a working GUI is not claimed |

Use these commands for the complete terminal coding-agent setup:

```bash
config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap --dry-run
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap
MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E ai-tools,ai-cli bootstrap status --missing
```

`ai-tools` is optional and is never implicitly selected by either application module. OpenCode global instructions follow `XDG_CONFIG_HOME`. `OPENCODE_CONFIG_DIR` is a user-owned override: when set, OpenCode uses it instead and essential-mise does not duplicate instructions there.

### pstack

`ai-cli` attempts to install [pstack](https://github.com/michael-denyer/pstack-claude) through the plugin mechanisms below. No pstack skills or plugin files are tracked here. `bin/pstack-plugins.sh` runs from the `post-repos` hook on every apply.

Pending routes print `PENDING` during apply and leave existing installations untouched. An apply can exit successfully while a route remains pending. Installation, repeat application, and plugin behavior for the other routes remain under review.

| Agent | Mechanism |
|---|---|
| Claude Code | **Pending.** Concurrent native marketplace and plugin operations can lose plugin enablement settings. Existing installations are left untouched and are not automatically updated. |
| Codex | **Pending.** Concurrent Codex configuration writes can lose changes. Existing installations are left untouched and are not automatically updated. |
| OMP | **Pending.** Native OMP support is in [upstream PR 396](https://github.com/michael-denyer/pstack-claude/pull/396) and requires a release. The installer leaves existing installations untouched. The earlier Pi package does not establish that OMP discovers the bundled skills. |
| OpenCode | **Pending.** The native Git-package integration in [upstream PR 397](https://github.com/michael-denyer/pstack-claude/pull/397) was guest-tested but needs a release. Automatic registration also remains blocked by OpenCode configuration-write races. Existing installations are untouched; inspect legacy links under `${XDG_CONFIG_HOME:-~/.config}/opencode/skills` manually because the old installer recorded no ownership. |
| Muse Code | `muse plugins install` from the clone's `plugins/pstack`; review runtime capabilities with `muse plugins inspect pstack` and approve the ones you trust |

The shared clone is `~/.local/share/pstack-claude`, a shallow native `[bootstrap.repos]` checkout of `main` that apply fast-forwards before the script runs. Muse Code comes from the Homebrew `muse-code` cask (Meta's official binary) through a real-Homebrew hook ([Homebrew exceptions](homebrew-exceptions.md)). Muse updates an installation from the same source path and refuses a different source. Existing Claude installations are left untouched. Muse approval is manual and bound to the reviewed capability definition; an upstream change can require review again. Restart an open Muse Code session after an update. Restart Claude Code or Codex after a manual plugin change. Codex hook trust is manual ([Manual actions](manual-actions.md)).

### i-have-adhd

`ai-cli` attempts to install [i-have-adhd](https://github.com/ayghri/i-have-adhd) through the plugin mechanisms below. `bin/adhd-plugins.sh` runs on every apply. No standalone i-have-adhd skill folder is installed.

The Claude Code, Codex, OMP and OpenCode routes print `PENDING` during apply and leave existing installations untouched. An apply can exit successfully while these routes remain pending. Installation, repeat application, and plugin behavior for the other routes remain under review.

| Agent | Mechanism |
|---|---|
| Claude Code | **Pending.** Concurrent native marketplace and plugin operations can lose plugin enablement settings. Existing installations are left untouched and are not automatically updated. |
| Codex | **Pending.** Concurrent Codex configuration writes can lose changes. Existing installations are left untouched and are not automatically updated. |
| OMP | **Pending.** OMP shares version caches across scopes. A user-scope install or upgrade can replace a cache another project uses. Equal version strings do not prove equal content. Existing installations are left untouched; see [OMP issue 16827](https://github.com/can1357/oh-my-pi/issues/16827). |
| OpenCode | **Pending manual review.** The hook does not install or change a loader because it cannot yet safely rule out another installation of the same plugin. Existing files and installations are preserved. |
| Muse Code | Installs or updates the checkout's Claude-compatible plugin from the same path; inspect and approve trusted runtime capabilities manually |

Codex 0.166.0 can lose concurrent marketplace registrations, so the hook does not install or update either plugin there. Review the native plugin manager before making manual changes; concurrent Codex configuration writes remain unsafe. Existing Claude installations are left untouched. Restart an agent session after installation or update. The plugin is opt-in unless its own always-on mode is enabled. Review Muse capabilities with `muse plugins inspect i-have-adhd` and approve the ones you trust. In the tested Muse 1.4.14 guest, always-on context required the flag at `~/.claude/.i-have-adhd-always`; a flag only under a custom `CLAUDE_CONFIG_DIR` did not activate it because Muse did not pass that variable to the hook. Other agents are not configured by this hook; Kimi's documented plugin installation is interactive, and the MiMo integration is pending upstream review.

### Other plugin routes

The `ai-cli` hook does not install these routes separately. Grok can discover the Claude installations; the others need additional setup or upstream support. Review a plugin and its source before installing it.

| Agent | pstack | i-have-adhd |
|---|---|---|
| Kimi Code | Native integration is pending [upstream PR 394](https://github.com/michael-denyer/pstack-claude/pull/394) and a release. | In a Kimi session, open `/plugins`, choose **Custom**, enter `https://github.com/ayghri/i-have-adhd`, review and confirm any prompts, and run `/reload`. Invoke `/skill:i-have-adhd:i-have-adhd`. The install is per-user. Use the plugin menu to update or remove it and reload; Kimi retains its managed copy after removal. |
| Grok Build | When the plugin is installed in Claude, Grok discovers it and exposes `/poteto-mode`. [Upstream PR 398](https://github.com/michael-denyer/pstack-claude/pull/398) proposes a model-followed routing hint; automatic routing is not guaranteed. | When the plugin is installed in Claude, Grok discovers it and exposes `/i-have-adhd`; the Claude always-on hook does not supply context on the tested Grok release. |
| Antigravity CLI | [Upstream PR 390](https://github.com/michael-denyer/pstack-claude/pull/390) is included in release 0.9.129; its routing and interactive skill delivered context in synthetic-provider guest controls. Automatic installation remains pending ownership and hook-contract checks. | The released plugin exposes the skill. The opt-in hook in [draft PR 277](https://github.com/ayghri/i-have-adhd/pull/277) delivered hook and interactive skill context in synthetic-provider guest sessions. |
| MiMo Code | Native integration is under investigation. | [Draft PR 276](https://github.com/ayghri/i-have-adhd/pull/276) remains pending; guest testing found it shadows existing same-name user or project skill resolution. |
| MiniMax Code | In mcode 0.7.4, a synthetic custom-provider session delivered routing and the explicit skill; disabled controls omitted them. | A synthetic custom-provider session delivered the explicit skill and the disabled control did not. The released always-on hook’s 30-second timeout is rejected by mcode 0.7.4. |

Grok 1.0.70 discovered both Claude-installed plugins in a disposable guest and honored each Claude disabled state in new sessions. No separate Grok installation was needed. Restart Grok after changing a plugin. Antigravity can replace an existing same-name plugin during installation. Check source and scope before changing an installation. Grok uninstall removes plugin data unless `--keep-data` is used. MiniMax Code is not provisioned by this module yet.

### RTK

`ai-tools` installs `rtk` and enables it for the installed agents listed below. The `post-dotfiles` hook runs `bin/rtk-agents.sh` after the agent instruction dotfiles are written. It skips `rtk` entirely when `rtk` is missing and skips any agent whose binary is missing, so `ai-tools` alone or `ai-cli` alone is safe. Repeating it changes no file.

| Agent | Mechanism |
|---|---|
| Claude Code | `rtk init -g --auto-patch`: `rtk hook claude` in `~/.claude/settings.json`, `~/.claude/RTK.md`. The `@RTK.md` line is already in the tracked `CLAUDE.md`, so rtk leaves it alone |
| OpenCode | `--opencode` is added when OpenCode is installed: plugin `~/.config/opencode/plugins/rtk.ts`, plus the Claude Code setup rtk documents for it |
| Codex | `rtk init -g --codex`: `rtk hook codex` in `~/.codex/hooks.json` next to other hooks, and `~/.codex/RTK.md`. Codex does not expand `@` imports in `AGENTS.md` ([source](https://github.com/openai/codex/blob/rust-v0.166.0/codex-rs/codex-home/src/instructions/mod.rs)), so the reference rtk writes there is inert; the hook does the work |
| Antigravity CLI | `rtk init -g --agent antigravity`: plugin `~/.gemini/config/plugins/rtk/` |
| OMP | `rtk init -g --agent omp`: extension `~/.omp/agent/extensions/rtk.ts` |
| Grok Build | rtk has no target. The script writes `~/.grok/hooks/rtk.json`, a `PreToolUse` hook that runs `rtk hook claude`. Grok's hook input and `updatedInput` reply follow Claude Code's |
| Muse Code | rtk has no target. The script writes a plugin bundle to `~/.local/share/essential-mise/rtk-muse-plugin`, installs it with `muse plugins install` and runs `muse plugins approve rtk`. Its `PreToolUse` hook runs `rtk hook claude` through `jq`, which drops `permissionDecisionReason` because Muse rejects it without a `permissionDecision` |
| Kimi Code | rtk only supports project scope (`rtk init --agent kimi`) and Kimi Code has no command-rewrite hook. The tracked `~/.kimi-code/AGENTS.md` carries the block that command writes, so Kimi is told to prefix commands with `rtk` |

Three instruction dotfiles carry the lines `rtk init` would write, so it finds them and leaves the files alone. `CLAUDE.md` ends with `@RTK.md`. `CODEX.md` and `KIMI.md` are mise templates that add their RTK lines only when `/opt/homebrew/bin/rtk` exists as mise renders them, so `ai-cli` without `ai-tools` never tells an agent to run a missing command. Copy-mode files with an extra line added by a second entry stay "differs" in `mise dot status`, so the lines are part of the source. After an rtk upgrade changes its Kimi block, copy the new block from a scratch `rtk init --agent kimi` into `KIMI.md`. Run `rtk init -g --show` and `rtk init -g --codex --show` to check the state, and `rtk init -g --uninstall` (with `--codex`, `--opencode` or `--agent <name>`) to remove an agent.

## `system-preferences`

Optional, user-scope macOS defaults for Dock, keyboard, Finder, locale, date/time, Siri, screensaver, verification-code deletion, TextInputMenu, and storage. `AppleFirstWeekday` and `AppleICUDateFormatStrings` use nested entries to preserve unrelated dictionary members.

Review the dry run carefully because Trash removal, verification-code deletion, storage optimization, locale, and Siri changes have real side effects. See [Manual actions](manual-actions.md) for snapshots and reversal. Symbolic hotkeys are not automated.

## `workspaces`

No active workspace module or placeholder is shipped. When real repository details exist, create ignored `config.workspaces.local.toml` directly with native `[bootstrap.repos]` and directories, then select `-E workspaces`. Do not commit private repository URLs or synthetic placeholders.
