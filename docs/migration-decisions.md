# Migration decisions

This is the exhaustive source-to-target inventory for `essential-mac` → `essential-mise`.

## Status and validation legend

- **NATIVE**: retained through a documented native mise declaration.
- **MANUAL**: intentionally left to the operator; no setup script is created.
- **DROP**: intentionally omitted from v1.
- **NEW**: added after the old inventory and not presented as migrated legacy state.

Validation uses native metadata/status/dry-run with mise **2026.10.23** during implementation. Actual installation, GUI behavior, and second-run convergence require the disposable-Mac gate in `docs/validation.md`. A declaration or upstream package listing alone is not installation proof.

The old generic components and sanitized legacy example have different provenance. They are audited separately and are not blindly merged.

## 1. All 41 root components

| Source component | Status | Destination and reason | Validation |
|---|---|---|---|
| `agents.claude-work` | DROP | No private account-specific structured-file state; no `~/.claude-work` writer | No private home is created or configured |
| `apps.agent-browser` | NATIVE + MANUAL + DROP | `brew:agent-browser` in `ai-tools`; runtime initialization is manual; old Chrome cleanup/retention is dropped | CLI metadata/status; `agent-browser install` remains a manual runtime step |
| `apps.amp` | REAL HOMEBREW + NATIVE CONFIG | Real-Homebrew `ampcode/tap/ampcode` hook in `ai-cli`; sanitized shared instructions replace old RTK/Caveman policy | Native mode-644 executable failed; real-Homebrew install and `amp --version` passed; `amp completions` tests |
| `apps.antigravity` | NATIVE + MANUAL | `brew-cask:antigravity-cli` in `ai-cli`, `brew-cask:antigravity` in `ai-desktop`; authentication manual | Separate package assertions; no cross-group configuration |
| `apps.chatgpt` | NATIVE + MANUAL | `brew-cask:chatgpt` in `ai-desktop`; authentication manual | Desktop-only package assertion |
| `apps.claude-code` | REAL HOMEBREW + NATIVE CONFIG | Real-Homebrew `claude-code@latest` in `ai-cli`; shared self-contained `~/.claude/CLAUDE.md`; native zsh status-line wrapper and executable script | Claude Code 2.1.312 and settings merge verified; exact status-line sample and subagent rules passed |
| `apps.claude-desktop` | NATIVE + MANUAL | `brew-cask:claude` in `ai-desktop`; authentication manual | Desktop-only package assertion |
| `apps.codex` | NATIVE + DROP | `brew-cask:codex` in `ai-cli`, `brew-cask:codexbar` in `ai-desktop`; shared Codex instructions; generated config/update/hook graph dropped | Separate package assertions; `CODEX.md` copy-mode check |
| `apps.cursor` | NATIVE + MANUAL | `brew-cask:cursor-cli` in `ai-cli`, `brew-cask:cursor` in `ai-desktop`; authentication manual | Separate package assertions |
| `apps.ghostty` | NATIVE + CHANGE | Stable `brew-cask:ghostty` in `terminal`; historical tip channel not selected; current config/theme copied | Nerd Font and fresh-shell UI checks |
| `apps.grok` | NATIVE + DROP | `brew-cask:grok-build` in `ai-cli`, native `macos-app:grok-bot` in `ai-desktop` using the current official DMG because the cask URL is outdated; shared instructions; generated completions dropped | Separate package assertions; no stale completion snapshot; official Grok DMG hash/signature verified |
| `apps.kimi` | NATIVE + CHANGE | `brew:kimi-code` in `ai-cli`, `brew-cask:kimi` in `ai-desktop`; shared instructions replace integration-only old text | Separate package assertions; no RTK/Caveman reference |
| `apps.omp` | NEW + REAL HOMEBREW | `can1357/tap/omp` through the documented real-Homebrew hook in `ai-cli`; native global instructions and YAML model merges | mise cannot execute `with_env`; real-Homebrew install proof; `omp config`, `omp models`, and instruction-file loading |
| `apps.opencode` | NATIVE + DROP | `brew:opencode` in `ai-cli`, `brew-cask:opencode-desktop` in `ai-desktop`; shared instructions; old JSONC/plugin integration dropped | Separate package assertions; no missing plugin paths |
| `apps.sketch` | NATIVE + MANUAL | `brew-cask:sketch` in `design`; Library registration manual | Package metadata; manual Library onboarding |
| `apps.surge` | NATIVE + MANUAL + DROP | `brew-cask:surge` in `network`; custom renderer, endpoint secrets, and privileged setup not ported | Native cask metadata/install; profile/proxy setup manual |
| `apps.telegram` | NATIVE + MANUAL | `mas:747648890` in `messengers`; sign-in manual | MAS availability/sign-in/rerun |
| `apps.warp` | NATIVE + MANUAL | `brew-cask:cloudflare-warp` in `network`; mode/disconnect automation dropped | Package metadata; WARP mode manual |
| `apps.zcode` | NATIVE + MANUAL | `brew-cask:zcode` in `ai-desktop`; authentication manual | Desktop-only package assertion |
| `category.admin` | NATIVE + NEW | Split into `admin`: Transmit, ARD, Disk Aid, iStatistica; Apple Configurator `mas:1037126344` added NEW | Per-app MAS/cask checks; no synthetic admin category engine |
| `category.ai` | NATIVE + DROP + APPROVED EXCEPTION | Selective split into `ai-tools` and `ai-cli`; old custom skills/MCP/Caveman/Firecrawl/statusline graph dropped; a narrowly approved Claude status-line script/wrapper is new | Independent environment checks; exact status-line output and settings merge verified |
| `category.apple-dev` | NATIVE + MANUAL | `mas:497799835` in `development-apple`; full Xcode onboarding manual | MAS sign-in and manual first launch |
| `category.china` | NATIVE | WeChat `mas:836500024` in `messengers` | MAS availability/sign-in |
| `category.dev` | NATIVE + MANUAL | `brew-cask:jetbrains-toolbox` in `development-apps`; IDE installation/settings manual | Cask metadata; Toolbox onboarding manual |
| `category.extra-browsers` | NATIVE | Firefox Developer Edition in `browsers`, Tor Browser in `privacy` | Separate package assertions |
| `category.infra` | NATIVE + DROP | `infrastructure` keeps OpenTofu, Ansible, linode-cli, gomplate; old deployment projects not ported | CLI metadata only; no provisioning |
| `category.k8s` | NATIVE | `kubernetes` keeps kubectl, kubectx, Helm | Formula metadata; cluster setup manual |
| `category.media` | NATIVE | `media` keeps Transmission, HandBrake, Plex; Blackmagic RAW remains MAS there | Separate cask/MAS checks |
| `category.preview` | NATIVE + CHANGE | Stable `brew-cask:zed` in `development-editors`; historical preview channel dropped | Stable cask metadata and JSONC merge |
| `host.gaps` | DROP + REAL HOMEBREW EXCEPTIONS | Native declarations are used where supported; five documented real-Homebrew exceptions are used for concrete mise incompatibilities | No `host.gaps` runtime or bridge exists; exception health checks, upgrade owners, and manual removal are documented |
| `host.git-lfs` | NATIVE + MANUAL | `brew:git-lfs` in standard `development`; initialization manual | Formula metadata; `git lfs install` manual |
| `host.network-time` | MANUAL | No native declarative equivalent approved for v1 | Manual Date & Time configuration |
| `host.tune` | MANUAL + DROP | Privileged sysctl/LaunchDaemon/pmset/systemsetup/Gatekeeper/login-window/Zoom/Little Snitch automation not ported | Manual host settings only |
| `macos.keyboard` | NATIVE + DROP | Named keyboard values in `system-preferences`; TIS/live input-source helper dropped | Stored keyboard values; live input-source setup manual |
| `macos.nested` | NATIVE + MANUAL + DROP | Safe scalar/dictionary values in `system-preferences`; Safari/OTP/TIS/appearance/Dock orchestration not ported | Nested dictionary preservation; behavior checks on disposable Mac |
| `optional.cask` | NATIVE | Redistributed by intent: editors, browsers, productivity, network, office, design, admin | Each optional declaration appears once by owner |
| `optional.mas` | NATIVE | Redistributed to development-apps, design, media, office | Each MAS declaration appears once by owner |
| `providers.mas` | NATIVE + CHANGE | No provider module; every MAS-owning environment declares identical `brew:mas` | Independent module checks and merged package state |
| `tools.java` | NATIVE | `development-java`: GraalVM, Gradle, Maven, VisualVM | Runtime/build-tool version checks and Homebrew OpenJDK review |
| `tools.rust` | NATIVE + DROP | `development-rust`: native `mr_boxington`; old Cargo wrapper, PATH shim, and release-age exceptions dropped | One Cargo strategy and `cargo --version` |
| `workspaces.owner` | MANUAL + DROP | No active workspace until real URLs exist; synthetic/private clones and secondary-remote orchestration omitted | Ignored local config only when real data is supplied |

## 2. Package and runtime declarations

Every declaration below is literal TOML. Repeated `brew:mas` is intentional independence for MAS modules; repeated Nerd Font is intentional independence for terminal/editors. All others have one functional owner.

| Module | Exact declaration | Status | Validation |
|---|---|---|---|
| `admin` | `"brew-cask:transmit" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `admin` | `"brew:mas" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `admin` | `"mas:409907375" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `admin` | `"mas:1007358832" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `admin` | `"mas:1447778660" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `admin` | `"mas:1037126344" = "latest"` | NEW | Apple lookup confirms `com.apple.configurator.ui`; disposable-host MAS install |
| `ai-cli` | Real Homebrew `/opt/homebrew/bin/brew install --cask claude-code@latest` in `post-packages` | REAL HOMEBREW + CHANGE | Stable cask's `bin/claude` target was missing; supported latest cask provides Claude Code 2.1.312 and a working CLI |
| `ai-cli` | `"brew-cask:codex" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-cli` | `"brew-cask:cursor-cli" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-cli` | `"brew-cask:antigravity-cli" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-cli` | `"brew:kimi-code" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-cli` | `"brew:opencode" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-cli` | `"brew-cask:grok-build" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-cli` | Real Homebrew `/opt/homebrew/bin/brew install ampcode/tap/ampcode` in `post-packages` | REAL HOMEBREW | Native installation left Amp CLI non-executable (mode 644); real Homebrew installed mode 555 and `amp --version` passed |
| `ai-cli` | Real Homebrew `/opt/homebrew/bin/brew install can1357/tap/omp` in `post-packages` | NEW + REAL HOMEBREW | mise 2026.10.23 cannot execute the formula's `with_env` completion helper; see [Homebrew exceptions](homebrew-exceptions.md) |
| `ai-desktop` | `"brew-cask:claude" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-desktop` | `"brew-cask:chatgpt" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-desktop` | `"brew-cask:cursor" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-desktop` | `"brew-cask:antigravity" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-desktop` | `"brew-cask:kimi" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-desktop` | `"brew-cask:opencode-desktop" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-desktop` | `macos-app:grok-bot` version `0.78.1`, official `/grokbot/` DMG, SHA-256 `0e575dfd2f638808f0487f88dec6bf53a871cac2939de7fe454981f927f9f0d9`, `Grok Bot.app` | NATIVE + OFFICIAL DIRECT | The cask's `/sand/` URL returned 403 through both package managers; the official page's current DMG passed hash, signature, notarization, and installation checks |
| `ai-desktop` | `"brew-cask:zcode" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-desktop` | `"brew-cask:codexbar" = "latest"` | NATIVE | Native metadata/status/dry-run; disposable-host install for third-party/privileged apps |
| `ai-desktop` | Real Homebrew `/opt/homebrew/bin/brew install --cask ampcode/tap/amp@beta` in `post-packages`; existing unmanaged app refused | NEW + REAL HOMEBREW | mise 2026.10.23 cannot evaluate the cask's `Array#second` DSL; current cask `1.0,996` uses the official DMG/checksum. No automatic adoption or second owner |
| `ai-tools` | `"brew:ripgrep" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:fd" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:fzf" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:ast-grep" = "latest"` | NEW | Formula API verified; `ast-grep`/`sg` binaries |
| `ai-tools` | `"brew:semgrep" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:jq" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:yq" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:dasel" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:sd" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:git-delta" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:eza" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:zoxide" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:tree" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:tokei" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:hyperfine" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:shellcheck" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:shfmt" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:universal-ctags" = "latest"` | NEW | Formula API verified; native install/status |
| `ai-tools` | `"brew:rtk" = "latest"` | NATIVE | Formula API verified; native install/status |
| `ai-tools` | `"brew:agent-browser" = "latest"` | NATIVE | Formula API verified; runtime initialization manual |
| `ai-tools` | Real Homebrew `/opt/homebrew/bin/brew install dmtrKovalenko/fff/fff-mcp` in `post-packages` | NEW + REAL HOMEBREW | Native installation left FFF non-executable (mode 644); real Homebrew installed mode 555 and `fff-mcp --healthcheck` passed |

### Third-party tap ownership

| Owning config | Literal declaration | Status and validation |
|---|---|---|
| `config.ai-cli.toml` | Real-Homebrew `ampcode/tap/ampcode` and `can1357/tap/omp` hooks | Amp CLI's native binary was non-executable; OMP requires real Homebrew because mise cannot execute `with_env`. No native tap or Ruby prerequisite remains |
| `config.ai-tools.toml` | Real-Homebrew `dmtrKovalenko/fff/fff-mcp` hook | Native binary was non-executable; real Homebrew health check passed. No native tap or Ruby prerequisite remains |
| `config.ai-desktop.toml` | Real-Homebrew `ampcode/tap/amp@beta` hook | Amp Desktop cask uses unsupported `Array#second`; no native tap or Ruby prerequisite remains |

| `browsers` | `"brew-cask:google-chrome" = "latest"` | NATIVE | Native cask metadata/install |
| `browsers` | `"brew-cask:google-chrome@dev" = "latest"` | NEW | Cask page verified; native metadata/install |
| `browsers` | `"brew-cask:firefox@developer-edition" = "latest"` | NATIVE | Native cask metadata/install |
| `design` | `"brew-cask:sketch" = "latest"` | NATIVE | Native cask metadata/install |
| `design` | `"brew:mas" = "latest"` | NATIVE | MAS prerequisite check |
| `design` | `"mas:1289583905" = "latest"` | NATIVE | MAS availability/sign-in |
| `development-apple` | `"brew:mas" = "latest"` | NATIVE | MAS prerequisite check |
| `development-apple` | `"mas:497799835" = "latest"` | NATIVE | MAS availability/sign-in; Xcode onboarding manual |
| `development-apps` | `"brew-cask:jetbrains-toolbox" = "latest"` | NATIVE | Native cask metadata/install |
| `development-apps` | `"brew-cask:yaak" = "latest"` | NATIVE | Native cask metadata/install |
| `development-apps` | `"brew-cask:tableplus" = "latest"` | NATIVE | Native cask metadata/install |
| `development-apps` | `"brew:mas" = "latest"` | NATIVE | MAS prerequisite check |
| `development-apps` | `"mas:6476452351" = "latest"` | NATIVE | MAS availability/sign-in |
| `development-apps` | `"mas:429449079" = "latest"` | NATIVE | MAS availability/sign-in |
| `development-containers` | `"brew-cask:orbstack" = "latest"` | NATIVE | Native cask metadata/install; runtime onboarding manual |
| `development-editors` | `"brew-cask:sublime-text" = "latest"` | NATIVE | Native cask metadata/install |
| `development-editors` | `"brew-cask:visual-studio-code" = "latest"` | NATIVE | Native cask metadata/install |
| `development-editors` | `"brew-cask:zed" = "latest"` | NATIVE + CHANGE | Stable channel; native metadata/install |
| `development-editors` | `"brew-cask:font-jetbrains-mono-nerd-font" = "latest"` | NATIVE | Independent font requirement; merged with terminal when combined |
| `development-java` | `java = "oracle-graalvm-25.0.4.1.1"` | NATIVE | Resolve exact runtime pin; install/invoke/persist checks |
| `development-java` | `"brew:gradle" = "latest"` | NATIVE | Formula metadata; Homebrew OpenJDK dependency reviewed |
| `development-java` | `"brew:maven" = "latest"` | NATIVE | Formula metadata; Homebrew OpenJDK dependency reviewed |
| `development-java` | `"brew-cask:visualvm" = "latest"` | NATIVE | Native cask metadata/install |
| `development-rust` | `rust = { version = "1.99.1", mr_boxington = true, components = ["llvm-tools"] }` | NATIVE | Resolve exact runtime pin; one native Cargo strategy |
| `development-rust` | `mr-boxington = { version = "1.24.1" }` | NATIVE | Resolve exact companion pin; no legacy wrapper |
| `development` | `"brew:gh" = "latest"` | NATIVE | Formula metadata; auth/setup-git manual |
| `development` | `"brew:git-lfs" = "latest"` | NATIVE | Formula metadata; initialization manual |
| `infrastructure` | `"brew:opentofu" = "latest"` | NATIVE | Formula metadata; no provisioning |
| `infrastructure` | `"brew:ansible" = "latest"` | NATIVE | Formula metadata; no provisioning |
| `infrastructure` | `"brew:linode-cli" = "latest"` | NATIVE | Formula metadata; no provisioning |
| `infrastructure` | `"brew:gomplate" = "latest"` | NATIVE | Formula metadata; no provisioning |
| `js` | `node = "26.9.0"` | NATIVE | Resolve exact runtime pin; install/invoke/persist checks |
| `js` | `bun = "1.4.4"` | NEW | `mise ls-remote bun` resolved; install/invoke/persist checks |
| `kubernetes` | `"brew:kubernetes-cli" = "latest"` | NATIVE | Formula metadata; cluster setup manual |
| `kubernetes` | `"brew:kubectx" = "latest"` | NATIVE | Formula metadata; cluster setup manual |
| `kubernetes` | `"brew:helm" = "latest"` | NATIVE | Formula metadata; cluster setup manual |
| `media` | `"brew-cask:vlc" = "latest"` | NATIVE | Native cask metadata/install |
| `media` | `"brew-cask:transmission" = "latest"` | NATIVE | Native cask metadata/install |
| `media` | `"brew-cask:handbrake-app" = "latest"` | NATIVE | Native cask metadata/install |
| `media` | `"brew-cask:plex-media-server" = "latest"` | NATIVE | Native cask metadata/install; server setup manual |
| `media` | `"brew:mas" = "latest"` | NATIVE | MAS prerequisite check |
| `media` | `"mas:1466185689" = "latest"` | NATIVE | MAS availability/sign-in |
| `media` | `"mas:897118787" = "latest"` | NATIVE | MAS availability/sign-in |
| `messengers` | `"brew:mas" = "latest"` | NATIVE | MAS prerequisite check |
| `messengers` | `"mas:747648890" = "latest"` | NATIVE | MAS availability/sign-in |
| `messengers` | `"mas:310633997" = "latest"` | NATIVE | MAS availability/sign-in |
| `messengers` | `"mas:803453959" = "latest"` | NATIVE | MAS availability/sign-in |
| `messengers` | `"mas:836500024" = "latest"` | NATIVE | MAS availability/sign-in |
| `messengers` | `"brew-cask:zoom" = "latest"` | NATIVE | Native cask metadata/install |
| `network` | `"brew-cask:surge" = "latest"` | NATIVE | Native cask metadata/install; profile setup manual |
| `network` | `"brew-cask:cloudflare-warp" = "latest"` | NATIVE | Native cask metadata/install; mode manual |
| `network` | `"brew-cask:tunnelblick" = "latest"` | NATIVE | Native cask metadata/install; VPN setup manual |
| `network` | `"brew-cask:whyfi" = "latest"` | NATIVE | Native cask metadata/install |
| `office` | `"brew-cask:microsoft-word" = "latest"` | NATIVE | Native cask metadata/install; license manual |
| `office` | `"brew-cask:microsoft-excel" = "latest"` | NATIVE | Native cask metadata/install; license manual |
| `office` | `"brew:mas" = "latest"` | NATIVE | MAS prerequisite check |
| `office` | `"mas:1055273043" = "latest"` | NATIVE | MAS availability/sign-in |
| `office` | `"mas:6740486871" = "latest"` | NATIVE | MAS availability/sign-in |
| `office` | `"mas:1350225911" = "latest"` | NATIVE | MAS availability/sign-in |
| `privacy` | `"brew-cask:tor-browser" = "latest"` | NATIVE | Native cask metadata/install |
| `privacy` | `"brew-cask:little-snitch" = "latest"` | NATIVE | Native cask metadata/install; extension/rules manual |
| `productivity` | `"brew-cask:notion" = "latest"` | NATIVE | Native cask metadata/install |
| `productivity` | `"brew-cask:grammarly-desktop" = "latest"` | NATIVE | Native cask metadata/install |
| `productivity` | `"brew-cask:speechify-voice-ai" = "latest"` | NATIVE | Native cask metadata/install |
| `productivity` | `"brew-cask:superwhisper" = "latest"` | NATIVE | Package only; all configuration manual |
| `productivity` | `"brew-cask:raindropio" = "latest"` | NATIVE | Native cask metadata/install |
| `productivity` | `"brew:mas" = "latest"` | NATIVE | MAS prerequisite check |
| `productivity` | `"mas:1462114288" = "latest"` | NATIVE | MAS availability/sign-in; Safari extension enablement manual |
| `productivity` | `"mas:6475002485" = "latest"` | NATIVE | MAS availability/sign-in |
| `security` | `"brew-cask:1password" = "latest"` | NATIVE | Native cask metadata/install; sign-in manual |
| `security` | `"brew-cask:1password-cli" = "latest"` | NATIVE | Native cask metadata/install; authorization manual |
| `security` | `"brew:mas" = "latest"` | NATIVE | MAS prerequisite check |
| `security` | `"mas:1569813296" = "latest"` | NATIVE | Apple lookup confirms `com.1password.safari`; enablement manual |
| `terminal` | `"brew-cask:ghostty" = "latest"` | NATIVE + CHANGE | Stable channel; native metadata/install |
| `terminal` | `"brew-cask:font-jetbrains-mono-nerd-font" = "latest"` | NATIVE | Matches Ghostty/Zed/Starship glyphs; shared with editors |
| `terminal` | `"brew:starship" = "latest"` | NATIVE | Sole prompt renderer |
| `terminal` | `"brew:bat" = "latest"` | NATIVE | Formula metadata/install |
| `terminal` | `"brew:watch" = "latest"` | NATIVE | Formula metadata/install |
| `terminal` | `"brew:just" = "latest"` | NATIVE | Formula metadata/install |
| `terminal` | `"brew:xh" = "latest"` | NATIVE | Formula metadata/install |
| `terminal` | `"brew:wget" = "latest"` | NATIVE | Formula metadata/install |
| `terminal` | `"brew:bash" = "latest"` | NATIVE | Formula metadata/install; stage-0 script still uses Apple `/bin/bash` |
| `terminal` | `"brew:nushell" = "latest"` | NATIVE | Formula metadata/install |
| `terminal` | `"brew:fish" = "latest"` | NEW | Formula page verified; native `auto` activation |
| `terminal` | `"brew:git-standup" = "latest"` | NATIVE | Formula metadata/install |
| `terminal` | `"brew:msedit" = "latest"` | NATIVE | Formula metadata/install |
| `terminal` | `"brew:buf" = "latest"` | NATIVE | Formula metadata/install |
| `terminal` | `"brew:gogcli" = "latest"` | NATIVE | Formula metadata/install |
| `utilities` | `"brew-cask:cleanshot" = "latest"` | NATIVE | Native cask metadata/install; login item path verified |
| `utilities` | `"brew-cask:bartender" = "latest"` | NATIVE | Native cask metadata/install; Bartender 7 path verified |
| `utilities` | `"brew-cask:homebrew-app" = "latest"` | NATIVE | Cask requires macOS >=26; target 27.2 satisfies |
| `utilities` | `"brew:mas" = "latest"` | NATIVE | MAS prerequisite check |
| `utilities` | `"mas:470158793" = "latest"` | NATIVE | MAS availability/sign-in |

## 3. Repository declarations

| Destination | Exact declaration | Status | Validation |
|---|---|---|---|
| `~/.oh-my-zsh` | `url = "https://github.com/ohmyzsh/ohmyzsh.git", ref = "0b50afdbfd8cbd0e03be68ecec4e1b5d3f55ef0a"` | NATIVE | Pinned native repo status; self-update disabled in zsh |
| `~/.oh-my-zsh/custom/plugins/zsh-autosuggestions` | `url = "https://github.com/zsh-users/zsh-autosuggestions.git", ref = "85919cd1ffa7d2d5412f6d3fe437ebdbeeec4fc5"` | NATIVE | Pinned nested checkout; Oh My Zsh ignores `custom/` so parent remains clean |

No active workspace repository is declared. Real future workspace URLs must use ignored `config.workspaces.local.toml` and are not represented by fake targets.

## 4. All 54 root `dotfiles/` files

| Source file | Status | Destination, reason, and validation |
|---|---|---|
| `superwhisper/modes/russian.json` | DROP | No Superwhisper files/directories deployed; old mode mechanism is not valid for intended setup |
| `superwhisper/modes/english.json` | DROP | Same: fully manual future Superwhisper configuration |
| `.zshrc` | DROP | No whole startup-file replacement; native foundation/terminal/Amp blocks own only declared behavior |
| `.zshenv` | DROP | Only foundation’s single `~/.local/bin` PATH line is native-managed |
| `.zprofile` | DROP | Only stage-0 Homebrew `shellenv` line is appended; old file is not reproduced |
| `.ssh/config` | DROP | Private hosts, identity agent, and endpoints are manual/private data |
| `.omp/agent/models.yml` | NATIVE | `~/.omp/agent/models.yml/shared` native YAML merge; validate with `omp models` |
| `.omp/agent/config.yml` | NATIVE | `~/.omp/agent/config.yml/shared` native YAML merge; validate roles with `omp config` |
| `.kimi-code/AGENTS.md` | NATIVE + CHANGE | Replaced by self-contained `GLOBAL_AGENTS.md` at `~/.kimi-code/AGENTS.md`; original was integration-only |
| `.grok/AGENTS.md` | NATIVE + CHANGE | Shared policy at same target, preserving substantive delegation/Git policy without RTK/Caveman |
| `.gitconfig` | NATIVE + MANUAL | Only listed nonidentity keys in `format = "ini"` merge; user and credential helpers remain manual (`gh auth setup-git`) |
| `.config/zellij/config.kdl` | DROP | No Zellij package selected; no unowned config deployed |
| `.config/zed/settings.json` | NATIVE + CHANGE | Renamed JSONC source; merge only approved layout/font/theme keys; `agent`/`agent_servers` dropped |
| `.config/starship.toml` | NATIVE | Copy to `~/.config/starship.toml`; Nerd Font and single prompt owner verified |
| `.config/rio/themes/generic-terminal.toml` | DROP | No Rio package selected |
| `.config/rio/config.toml` | DROP | No Rio package selected |
| `.config/opencode/plugins/rtk.ts` | DROP | References an unselected integration/plugin |
| `.config/opencode/opencode.jsonc` | DROP | Contains Caveman plugin references; no stale config deployed |
| `.config/opencode/AGENTS.md` | NATIVE + CHANGE | Shared self-contained policy at same target; Caveman/RTK paragraph removed |
| `.config/jackin/workspaces/velnor.toml` | DROP | No Jackin selected; workspace/private data is not activated |
| `.config/jackin/workspaces/scentbird.toml` | DROP | Same |
| `.config/jackin/workspaces/scentbird-ai.toml` | DROP | Same |
| `.config/jackin/workspaces/renovate-rust-experiement.toml` | DROP | Same; historical misspelling left unchanged in source audit only |
| `.config/jackin/workspaces/parallax.toml` | DROP | Same |
| `.config/jackin/workspaces/jackin.toml` | DROP | Same |
| `.config/jackin/workspaces/jackin-roles.toml` | DROP | Same |
| `.config/jackin/workspaces/jackin-dev.toml` | DROP | Same |
| `.config/jackin/workspaces/holla.toml` | DROP | Same |
| `.config/jackin/workspaces/chainargos.toml` | DROP | Same |
| `.config/jackin/workspaces/chainargos-satoshi.toml` | DROP | Same |
| `.config/jackin/workspaces/chainargos-platform.toml` | DROP | Same |
| `.config/jackin/workspaces/chainargos-blockchain-nodes.toml` | DROP | Same |
| `.config/jackin/config.toml` | DROP | No Jackin selected; host attach/private workspace configuration not activated |
| `.config/git/ignore` | NATIVE | Copy to `~/.config/git/ignore` |
| `.config/ghostty/themes/generic-terminal` | NATIVE | Copy to matching Ghostty theme path |
| `.config/ghostty/config` | NATIVE | Copy to `~/.config/ghostty/config`; Nerd Font installed |
| `.config/gh/hosts.yml` | DROP | Authentication/account data is not shipped |
| `.config/gh/config.yml` | NATIVE | YAML merge of listed nonsecret keys; scalar settings and alias validated separately |
| `.config/fish/functions/.keep` | DROP | Empty directory marker is not useful configuration |
| `.config/fish/fish_variables` | DROP | Application-owned state is not replaced |
| `.config/fish/config.fish` | DROP | Placeholder would suppress native Fish `auto` activation; additive Amp block only |
| `.config/fish/conf.d/.keep` | DROP | Empty directory marker is not useful configuration |
| `.config/fish/completions/orbctl.fish` | DROP | Absolute-path stub to old OrbStack payload, not standalone completions |
| `.config/fish/completions/kubectl.fish` | DROP | Absolute-path stub to old OrbStack payload, not standalone completions |
| `.config/fish/completions/grok.fish` | DROP | Large generated snapshot can become stale; application supplies completions |
| `.config/fish/completions/docker.fish` | DROP | Absolute-path stub to old OrbStack payload, not standalone completions |
| `.config/caveman/providers/zai.yaml` | DROP | Caveman/provider integration intentionally not ported |
| `.config/caveman/providers/minimax.yaml` | DROP | Caveman/provider integration intentionally not ported |
| `.config/caveman/providers/kimi.yaml` | DROP | Caveman/provider integration intentionally not ported |
| `.config/caveman/config.json` | DROP | Caveman integration intentionally not ported |
| `.config/amp/AGENTS.md` | NATIVE + CHANGE | Shared self-contained policy at same target; Amp CLI is now explicitly installed |
| `.config/alacritty/alacritty.toml` | DROP | No Alacritty package selected |
| `.codex/AGENTS.md` | NATIVE + CHANGE | `CODEX.md` shared policy plus Codex co-author line; no RTK/Caveman references |
| `.claude/CLAUDE.md` | NATIVE + CHANGE | `GLOBAL_AGENTS.md` at `~/.claude/CLAUDE.md`; RTK lines, `@RTK.md`, and Caveman text removed |

## 5. All 33 sanitized legacy-example dotfiles

These rows preserve provenance. “NATIVE” uses the reviewed current root source, not a blind merge with the sanitized example.

| Source file | Status | Destination, reason, and validation |
|---|---|---|
| `superwhisper/modes/russian.json` | DROP | Fully excluded, matching root decision |
| `superwhisper/modes/english.json` | DROP | Fully excluded, matching root decision |
| `.zshrc` | DROP | No whole startup file; native blocks only |
| `.zshenv` | DROP | Foundation PATH line only |
| `.zprofile` | DROP | Stage-0 Homebrew line only |
| `.ssh/config` | DROP | Private/manual SSH configuration |
| `.grok/AGENTS.md` | DROP | Old agent policy not ported; reviewed current shared source is used instead |
| `.gitconfig` | DROP | Separate sanitized provenance is not merged; only root nonidentity keys are used |
| `.config/zellij/config.kdl` | DROP | No selected owner app |
| `.config/zed/settings.json` | DROP | Legacy variant not merged; root JSONC-approved subset is used |
| `.config/starship.toml` | NATIVE | Current root `starship.toml` copied; legacy file not blindly merged |
| `.config/rio/themes/essential-mac.toml` | DROP | No Rio package selected |
| `.config/rio/config.toml` | DROP | No Rio package selected |
| `.config/opencode/plugins/rtk.ts` | DROP | Unselected integration |
| `.config/opencode/opencode.json` | DROP | Legacy JSON/plugin integration not ported |
| `.config/opencode/AGENTS.md` | NATIVE | Current shared policy used instead of legacy text |
| `.config/jackin/workspaces/example.toml` | DROP | Synthetic/private workspace data is not activated |
| `.config/jackin/config.toml` | DROP | No Jackin selected |
| `.config/git/ignore` | NATIVE | Current root source copied |
| `.config/ghostty/themes/dark` | DROP | Legacy theme variant not merged; current `generic-terminal` used |
| `.config/ghostty/config` | NATIVE | Current root Ghostty config used, not a three-way merge |
| `.config/gh/hosts.yml` | DROP | Authentication data not shipped |
| `.config/gh/config.yml` | NATIVE | Current root nonsecret YAML merge used |
| `.config/fish/functions/.keep` | DROP | Empty marker |
| `.config/fish/fish_variables` | DROP | Application state not replaced |
| `.config/fish/config.fish` | DROP | Native Fish activation owns the file’s activation block |
| `.config/fish/conf.d/.keep` | DROP | Empty marker |
| `.config/fish/completions/example.fish` | DROP | Synthetic completion is not a real current app resource |
| `.config/caveman/config.json` | DROP | Caveman integration not ported |
| `.config/amp/AGENTS.md` | NATIVE | Current shared policy used now that Amp CLI is selected |
| `.config/alacritty/alacritty.toml` | DROP | No Alacritty package selected |
| `.codex/AGENTS.md` | NATIVE | Current `CODEX.md` used |
| `.claude/CLAUDE.md` | NATIVE | Current `GLOBAL_AGENTS.md` used |

## 6. Approved `system-preferences` entries

All are optional, user-scope (`host = "any"`), and have both stored-state and behavior verification. `docs/manual-actions.md` gives each effect and reversal. Deselecting the module does not revert values.

| Domain/key | Exact value/declaration | Status | Validation |
|---|---|---|---|
| `com.apple.dock autohide` | `true` | NATIVE | Stored boolean; Dock behavior after restart |
| `com.apple.dock orientation` | `"bottom"` | NATIVE | Stored string; Dock position after restart |
| `com.apple.dock tilesize` | `58` | NATIVE | Stored integer; icon size after restart |
| `com.apple.dock magnification` | `false` | NATIVE | Stored boolean; magnification state |
| `com.apple.dock show-recents` | `false` | NATIVE | Stored boolean; recents hidden |
| `com.apple.dock mru-spaces` | `false` | NATIVE | Stored boolean; Spaces ordering |
| `NSGlobalDomain KeyRepeat` | `1` | NATIVE | Stored integer; actual key repeat |
| `NSGlobalDomain InitialKeyRepeat` | `10` | NATIVE | Stored integer; actual initial delay |
| `NSGlobalDomain ApplePressAndHoldEnabled` | `false` | NATIVE | Stored boolean; held-key behavior |
| `com.apple.finder FXRemoveOldTrashItems` | `true` | NATIVE + destructive review | Stored boolean; 30-day permanent Trash deletion review |
| `com.apple.finder CreateDesktop` | `true` | NATIVE | Stored boolean; Desktop icons after Finder restart |
| `com.apple.dock mineffect` | `"genie"` | NATIVE | Stored string; minimize effect |
| `com.apple.dock minimize-to-application` | `false` | NATIVE | Stored boolean; minimize destination |
| `com.apple.dock launchanim` | `true` | NATIVE | Stored boolean; launch animation |
| `com.apple.dock show-process-indicators` | `true` | NATIVE | Stored boolean; running indicators |
| `com.apple.dock workspaces-auto-swoosh` | `false` | NATIVE | Stored boolean; Space switching behavior |
| `com.apple.dock expose-group-apps` | `false` | NATIVE | Stored boolean; Mission Control grouping |
| `com.apple.dock enter-mission-control-by-top-window-drag` | `false` | NATIVE | Stored boolean; drag behavior |
| `com.apple.dock wvous-tl-corner` | `0` | NATIVE + behavior review | Stored integer; top-left hot corner off |
| `com.apple.dock wvous-tr-corner` | `2` | NATIVE + behavior review | Stored integer; top-right Mission Control |
| `com.apple.dock wvous-bl-corner` | `4` | NATIVE + behavior review | Stored integer; bottom-left Desktop |
| `com.apple.dock wvous-br-corner` | `11` | NATIVE + behavior review | Stored integer; bottom-right application windows |
| `com.apple.dock wvous-tl-modifier` | `0` | NATIVE | Stored integer; modifier cleared |
| `com.apple.dock wvous-tr-modifier` | `0` | NATIVE | Stored integer; modifier cleared |
| `com.apple.dock wvous-bl-modifier` | `0` | NATIVE | Stored integer; modifier cleared |
| `com.apple.dock wvous-br-modifier` | `0` | NATIVE | Stored integer; modifier cleared |
| `com.apple.spaces spans-displays` | `false` | NATIVE | Stored boolean; display/Spaces behavior |
| `com.apple.WindowManager GloballyEnabled` | `false` | NATIVE | Stored boolean; Stage Manager off |
| `com.apple.WindowManager EnableStandardClickToShowDesktop` | `true` | NATIVE | Stored boolean; click-to-Desktop behavior |
| `com.apple.WindowManager EnableTilingByEdgeDrag` | `false` | NATIVE | Stored boolean; edge tiling off |
| `com.apple.WindowManager EnableTopTilingByEdgeDrag` | `false` | NATIVE | Stored boolean; top-edge tiling off |
| `com.apple.WindowManager EnableTilingOptionAccelerator` | `false` | NATIVE | Stored boolean; Option accelerator off |
| `NSGlobalDomain AppleInterfaceStyleSwitchesAutomatically` | `true` | NATIVE | Stored boolean; automatic appearance |
| `NSGlobalDomain AppleLocale` | `"en_US"` | NATIVE + behavior review | Stored string; actual region behavior |
| `NSGlobalDomain AppleMetricUnits` | `true` | NATIVE | Stored boolean; metric units |
| `NSGlobalDomain AppleMeasurementUnits` | `"Centimeters"` | NATIVE | Stored string; measurement unit |
| `NSGlobalDomain AppleTemperatureUnit` | `"Celsius"` | NATIVE | Stored string; temperature unit |
| `NSGlobalDomain AppleICUForce24HourTime` | `true` | NATIVE | Stored boolean; 24-hour display |
| `NSGlobalDomain AppleLiveTextEnabled` | `true` | NATIVE | Stored boolean; Live Text |
| `NSGlobalDomain AppleActionOnDoubleClick` | `"Maximize"` | NATIVE | Stored string; double-click behavior |
| `NSGlobalDomain AppleMiniaturizeOnDoubleClick` | `false` | NATIVE | Stored boolean; double-click minimize off |
| `NSGlobalDomain AppleWindowTabbingMode` | `"fullscreen"` | NATIVE | Stored string; tab behavior |
| `NSGlobalDomain NSCloseAlwaysConfirmsChanges` | `false` | NATIVE | Stored boolean; close-confirmation behavior |
| `NSGlobalDomain NSQuitAlwaysKeepsWindows` | `false` | NATIVE | Stored boolean; windows not restored on quit |
| `NSGlobalDomain NSAutomaticSpellingCorrectionEnabled` | `false` | NATIVE | Stored boolean; spelling correction off |
| `NSGlobalDomain NSAutomaticCapitalizationEnabled` | `false` | NATIVE | Stored boolean; capitalization off |
| `NSGlobalDomain NSAutomaticInlinePredictionEnabled` | `false` | NATIVE | Stored boolean; inline prediction off |
| `NSGlobalDomain NSSmartReplyEnabled` | `true` | NATIVE | Stored boolean; suggested replies on |
| `NSGlobalDomain NSAutomaticPeriodSubstitutionEnabled` | `false` | NATIVE | Stored boolean; period substitution off |
| `NSGlobalDomain NSAutomaticQuoteSubstitutionEnabled` | `false` | NATIVE | Stored boolean; quote substitution off |
| `NSGlobalDomain NSAutomaticDashSubstitutionEnabled` | `false` | NATIVE | Stored boolean; dash substitution off |
| `NSGlobalDomain KB_DoubleQuoteOption` | `"“abc”"` | NATIVE | Stored string; double-quote style |
| `NSGlobalDomain KB_SingleQuoteOption` | `"‘abc’"` | NATIVE | Stored string; single-quote style |
| `NSGlobalDomain NSTableViewDefaultSizeMode` | `2` | NATIVE | Stored integer; medium sidebar/icon size |
| `NSGlobalDomain AppleReduceDesktopTinting` | `false` | NATIVE | Stored boolean; tinting behavior |
| `NSGlobalDomain AppleShowScrollBars` | `"Automatic"` | NATIVE | Stored string; scroll-bar behavior |
| `NSGlobalDomain AppleLanguages` | `["en-US"]` | NATIVE + behavior review | Stored array; actual language list |
| `NSGlobalDomain AppleFirstWeekday.path = ["gregorian"]` | `2` via `[[bootstrap.macos.defaults_entries]]` | NATIVE + preservation | Nested edit; unrelated calendar members preserved |
| `NSGlobalDomain AppleICUDateFormatStrings.path = ["1"]` | `"y-MM-dd"` via `[[bootstrap.macos.defaults_entries]]` | NATIVE + preservation | Nested edit; unrelated date-format members preserved |
| `com.apple.assistant.support Assistant Enabled` | `false` | NATIVE | Stored boolean; assistant behavior |
| `com.apple.Siri StatusMenuVisible` | `false` | NATIVE | Stored boolean; Siri menu hidden |
| `com.apple.Siri VoiceTriggerUserEnabled` | `false` | NATIVE | Stored boolean; voice trigger off |
| `com.apple.screensaver askForPassword` | `true` | NATIVE | Stored boolean; password required |
| `com.apple.screensaver askForPasswordDelay` | `300` | NATIVE | Stored integer; five-minute delay |
| `com.apple.onetimepasscodes DeleteVerificationCodes` | `true` | NATIVE + destructive review | Stored boolean; used-code deletion behavior |
| `com.apple.TextInputMenu visible` | `true` | NATIVE | Stored boolean; input menu visible |
| `com.apple.bird optimize-storage` | `true` | NATIVE + data-availability review | Stored boolean; storage/file-availability behavior |
| `com.apple.symbolichotkeys AppleSymbolicHotKeys` | No declaration | MANUAL | Existing dictionaries/parameters are not synthesized on clean Macs |

### Login items

| Application | Exact value | Status | Validation |
|---|---|---|---|
| CleanShot | `/Applications/CleanShot X.app` in additive `[bootstrap.macos.login_items]`, `exclusive = false` | NATIVE | App exists after cask install; additive login-item readback on disposable Mac |
| Bartender | `/Applications/Bartender 7.app` in additive `[bootstrap.macos.login_items]`, `exclusive = false` | NATIVE | App exists after cask install; additive login-item readback on disposable Mac |

## 7. Significant behavior dispositions

| Behavior | Status | Destination and validation |
|---|---|---|
| CLT/Git, official Homebrew, pinned mise, inert adoption, foundation activation | NATIVE via sole `bootstrap.sh` | First-run/rerun, failed download, cancellation, wrong-origin, alternate XDG, and fresh-shell tests |
| First adoption versus stage-0 rerun | NATIVE via script branches | Existing valid checkout skips adoption; `-E foundation` prevents optional reapplication |
| Shell activation | NATIVE | `~/.local/bin` line in `~/.zshenv`, zsh activation, Fish `auto`; no whole startup-file copy |
| Oh My Zsh prompt/update behavior | NATIVE + CHANGE | `ZSH_THEME=""`, Starship sole prompt, `zstyle ':omz:update' mode disabled`; pinned repos |
| Runtime installation versus activation | NATIVE | One-off `-E bootstrap`, one-off `-E exec`, persistent `miserc.local.toml`; unrelated-directory checks |
| Rust Cargo behavior | NATIVE + DROP | Native `mr_boxington` only; legacy wrapper/PATH shim/release-age exceptions dropped |
| Java runtime behavior | NATIVE + documented dependency | GraalVM active despite Homebrew OpenJDK; `java`, `mvn`, `gradle` checked on disposable Mac |
| GitHub/Git identity and credentials | MANUAL | Real identity deliberately configured; `gh auth login` and `gh auth setup-git`; no hosts.yml or helper reset migration |
| Git LFS initialization | MANUAL | `git lfs install`; no custom hook |
| AI instruction distribution | NATIVE + CHANGE | Exact `GLOBAL_AGENTS.md`/`CODEX.md` copy declarations; no RTK/FFF/Caveman/missing-plugin references |
| OMP model/provider configuration | NATIVE + MANUAL validation | YAML merges plus `omp config`/`omp models`; stale roles/models/routes dropped; no credentials |
| Amp completions | NATIVE | Guarded zsh `compinit` and interactive Fish blocks; tests with and without `terminal` |
| Agent-browser runtime | MANUAL | `agent-browser install`; no old cleanup/retention automation |
| FFF MCP | Conditional NEW | Keep only after native evaluation and real install; otherwise MANUAL with recorded error; no auto-MCP config |
| Superwhisper configuration | DROP | No files/directories; all future setup manual |
| Homebrew/mise cask ownership | NATIVE policy | Mise-owned casks remain mise-managed; Homebrew-owned casks remain Homebrew-managed; no metadata handoff |
| Package pruning | MANUAL + explicit approval | Broad maintenance operation, never automatic after unapply; manager and full dry-run reviewed |
| Workspaces | MANUAL until real data | No fake repos; ignored `config.workspaces.local.toml` only after real URLs/ownership are supplied |
| Managed mobileconfig experiments | DROP | No profiles, validation engine, installation, or MDM enrollment in this repository |
| OpenTofu/Ansible Singapore deployment project | DROP | Only infrastructure CLIs are native; separate operational project not migrated |
| Private owner identities/workspaces/credentials | DROP | No synthetic owner data, private URLs, tokens, or sanitized private home restoration |
| Historical examples/golden plans/framework tests | DROP | Not active machine state; new native validation only |

## 8. Completion audit

- Root components: **41/41** mapped.
- Root dotfiles: **54/54** mapped individually.
- Sanitized legacy-example dotfiles: **33/33** mapped individually with separate provenance.
- Package/runtime/repository/preference/login-item declarations: each has an exact native entry or an explicit MANUAL/DROP/NEW disposition.
- No source behavior is silently preserved through a second orchestrator.
