# Validation record

**Status: PRE-RELEASE. The disposable-Mac release gate has not passed.** The public `main` branch is not presented as an end-to-end-tested release source. Use a reviewed commit-pinned launcher for any real setup.

## Environment and safety boundary

Validation uses the official mise **2026.10.23 macOS arm64** binary, downloaded to an isolated path with SHA-256 verification. The checksum was `666b62cf0951b0a8ffccbfa832bfad8e19420d2bd5137b0458ce181f4225e835` before the binary was renamed to `mise` for execution.

Checks use temporary HOME, XDG, cache, data, and state directories. During these checks, no real-host adoption, package installation, macOS preference change, remote creation, push, credential retrieval, or infrastructure provisioning was performed. Metadata/status commands may inspect current host package state and download only into isolated caches. Repository publication was a separate later action.

The only intentional implementation-host configuration changes were the three approved global Git identity settings.

## Tart and Cua Driver verification

The current runtime work uses Tart **2.41.1** on an Apple Silicon macOS **27.2 (26B123)** host with 64 GB RAM. The host has Command Line Tools, not full Xcode. No application under test is launched on the host, no host directory is shared, clipboard sharing is disabled, and only synthetic guest accounts/data are used. The guest uses NAT networking, which is not complete security isolation.

| Check | Result |
|---|---|
| Fresh Apple IPSW | PASS: Tart downloaded `UniversalMac_27.2_26B123_Restore.ipsw`; no older guest was substituted |
| Fresh `admin`/`admin` baseline | PASS: `essential-mise-fresh-baseline` was recreated from the Apple IPSW, provisioned with username/password `admin`, verified as 27.2/26B123 arm64 with SIP and Gatekeeper enabled, synced, and stopped; 4 CPUs, 8192 MB RAM, 100 GB disk |
| Cua Driver package | PASS on the new automation baseline: Homebrew `cuadriver` 0.38.0, Developer ID signature, notarization, `codesign --verify --deep --strict`, and Gatekeeper acceptance |
| Guest Homebrew/CLT | PASS on the new automation baseline: Homebrew 7.0.11 and Command Line Tools for Xcode 27.0 |
| Disposable stage 0 | PASS at `cc7f478f19790482a8a3134384e55701788c9f68` on the recreated `admin`/`admin` clone: public commit-pinned launcher, optional `ai-cli` selection rerun, wrong-origin refusal, and foundation-scoped fresh-shell doctor |
| Disposable standard modules | PASS on the final `admin`/`admin` clone: foundation, terminal, development packages/apps, pinned repositories, copied dotfiles, second-run convergence, and zsh/Fish activation. Ghostty 1.4.0 is verified through its app-bundle executable because the cask declares no binary link |
| AI Desktop including real-Homebrew Amp | PASS: full `ai-desktop` preview, apply, second run, and cask ownership verification succeeded. Amp Desktop `1.0,996` is registered to real Homebrew. The separate unmanaged-app hook refused without taking ownership, and the disposable conflict VM was removed |
| Full AI CLI/tools | PASS: preview, apply, status, second-run convergence, and CLI checks. Real Homebrew owns Amp CLI, OMP, and FFF; `amp --version`, `omp --version`, and `fff-mcp --healthcheck` passed |
| Grok Bot official DMG fallback | PASS: native `macos-app:grok-bot` installed from the official `/grokbot/` URL with exact SHA-256 `0e575dfd2f638808f0487f88dec6bf53a871cac2939de7fe454981f927f9f0d9`; signature/notarization verified. The outdated cask `/sand/` URL returned 403 through both mise and real Homebrew |
| Complete unmodified dry run | PASS: all 27 environments ran in one unmodified `mise bootstrap --dry-run` on the final guest, with no package exclusions |
| macOS preferences | PASS: preview, apply, stored status, Dock values, and preservation of synthetic unrelated global/nested values, including `AppleFirstWeekday.syntheticKeep` and unrelated `AppleICUDateFormatStrings` members |
| Ghostty application UI | PASS: a real guest window rendered Starship/Nerd Font correctly; Cua typed and verified `ESSENTIAL_MISE_UI_OK` in both the accessibility output and screenshot |
| Amp Desktop UI startup | **BLOCKED**: the installed Amp process stalls at `_dyld_start` before application code, with no window, crash report, or useful log. The installed executable is byte-identical to the official DMG, so this is not a Homebrew copy issue. Package ownership/install succeeded, but Amp GUI behavior is unverified on macOS 27.2 |
| True no-Homebrew/no-CLT first run | PASS at `0d57f580d0024088422c1e233fd6c527bfdd54f3`: a fresh guest with neither Homebrew nor CLT completed Apple's Install/license flow through Cua, Homebrew and pinned mise installation, public adoption, foundation activation, and a fresh-shell doctor with `No problems found`. Logs, final Terminal screenshot, and MCP evidence were collected before deleting only the disposable clone |
| Guest Cua consent/UI/MCP | PASS: Accessibility, Screen Recording, and direct capture verified under CuaDriver's own identity; MCP initialization, 62 tools, guest app listing, and real desktop captures succeeded over SSH. Packaged `smoke-ui.py` used accessibility tokens to calculate 6×7 and verified 42 in both the tree and screenshot |
| Disposable clone lifecycle | PASS: workflow-owned deletion, new clone from the stopped verified baseline, SSH restoration, Cua grants after cloning, and MCP evidence. Baselines and unrelated/unregistered targets were refused |

The final automation baseline is `essential-mise-automation-baseline`, stopped after readiness verification. The disposable `essential-mise-test-current` inherited Cua grants and passed `check-vm-ready.sh` plus `smoke-mcp.sh`. Codex registration `essential-mise-vm` targets the disposable launcher with no host-local fallback. Guest screenshots are stored under `/tmp/essential-mise-evidence-automation` and `/tmp/essential-mise-mcp-essential-mise-test-current-desktop.png` on the host; they contain only synthetic guest state.

The fresh and automation baselines are distinct. Clone startup includes explicit restoration and verification of the dedicated guest public key after `authorized_keys` was observed missing in a clone. The owner-auth helper uses normal SSH/sudo prompts; it does not disable security controls or edit TCC. The reviewed Homebrew installer SHA-256 is `5f333bbe53bc490e51e7ccb1df8779b3dd6ee73a1a7379efda216edb08ccb148`. Every test guest uses only the synthetic `admin`/`admin` credential; no unique generated guest passwords remain in the workflow.

Earlier logs are preserved under `/tmp/essential-mise-evidence-pre-admin` on the implementation host. Reusable setup, clone, scenario, MCP, evidence, and cleanup commands are documented in `scripts/vm/README.md`.

## Completed checks

| Check | Result |
|---|---|
| `/bin/bash -n bootstrap.sh` | PASS |
| `git diff --check` | PASS |
| Isolated mise version | PASS: `2026.10.23 macos-arm64 (2026-10-29)` |
| Base `mise config` | PASS: only inert `config.toml` loaded |
| Base `mise bootstrap --dry-run` | PASS: no optional resources; tools empty |
| Base `mise bootstrap status --json` | PASS: every resource collection empty |
| Base `mise dot status --json` | PASS: no dotfiles or tracked history |
| All 27 environment `bootstrap status --json` checks | PASS after fixes: foundation, terminal, development, security, js, development-rust, development-java, development-apps, development-containers, development-editors, development-apple, kubernetes, infrastructure, utilities, messengers, browsers, privacy, productivity, office, design, media, network, admin, ai-tools, ai-cli, ai-desktop, system-preferences |
| Full combined `bootstrap --dry-run` | **PASS on the final `admin`/`admin` guest:** all 27 environments completed one unmodified preview with no excluded declarations. The earlier isolated host-only PARTIAL result is superseded |
| Dotfile source references | PASS: all exist; no source symlinks |
| Zed JSONC content | PASS: valid after trailing-comma normalization; no `agent` or `agent_servers` |
| Git INI source | PASS: parses with Python `ConfigParser`; no identity or credential-helper sections |
| Migration inventory | PASS: all 41 components, 54 root dotfiles, and 33 legacy-example dotfiles are individually mapped |
| High-confidence secret scan | PASS: no matches for 1Password references, GitHub tokens, AWS keys, or private-key blocks |
| AI instruction dependency scan | PASS: no RTK/FFF/Caveman/agent-browser/missing-plugin references in approved shared instructions |
| Repeated package ownership audit | PASS: intentional Nerd Font and independent MAS duplicates, documented real-Homebrew exceptions, and one official direct Grok app; no second owner or Ruby prerequisite remains |
| Amp Desktop ownership declaration and conflict handling | PASS: real-Homebrew `ampcode/tap/amp@beta` hook; clean install passed, and a separate synthetic unmanaged app triggered refusal without marker modification or cask registration. No adoption declaration exists |
| FFF real-Homebrew installation | PASS: the native install's mode-644 binary failed with permission denied; real Homebrew installed mode 555 and `fff-mcp --healthcheck` reported `All checks passed` |
| Amp CLI real-Homebrew installation | PASS on disposable macOS 27.2: the native install's mode-644 `amp` failed with permission denied; real Homebrew reinstalled it with mode 555 and `amp --version` succeeded |
| Third-party tap prerequisites | No Ruby prerequisite remains. Native `[tools]` Ruby fixed preview but did not make Amp CLI's raw binary executable; all demonstrated incompatible formulae now use real Homebrew |
| `claude-code` cask metadata | PASS: Homebrew API returned HTTP 200 |

## Issues found and fixed

1. **Third-party formula compatibility and Ruby:** native preview needed Ruby 3+, and `[tools]` Ruby made that preview work, but actual Amp CLI and FFF binaries were left non-executable (mode 644), while OMP used unsupported `with_env`. Native formula declarations and the Ruby prerequisite were removed. Real Homebrew now owns Amp CLI, OMP, and FFF; Amp/FFF health checks passed with executable mode 555. Every exception is recorded in [Homebrew exceptions](homebrew-exceptions.md).
2. **Real-Homebrew compatibility exceptions:** Amp Desktop's cask failed with `undefined method 'second' for an instance of Array`; OMP's formula failed with `formula uses with_env (install-time helper), which mise's source-build shim does not support`. Native declarations were removed and real-Homebrew hooks now own Amp Desktop, Amp CLI, OMP, and FFF. Clean installs and full AI convergence passed on the `admin`/`admin` guest; the separate existing-Amp conflict test remains. See [Homebrew exceptions](homebrew-exceptions.md).
3. **Historical isolated-host cask conflict / partial combined preview:** the original isolated test excluded `brew-cask:claude-code` to avoid an existing-host ownership conflict. That host-only PARTIAL result is retained for context, not as the current result. The final `admin`/`admin` guest completed the full unmodified 27-environment dry run with no exclusions.

   The original failing check was:

   ```bash
   env -i PATH=/tmp/essential-mise-ruby-data/installs/ruby/4.0.9/bin:/usr/bin:/bin:/opt/homebrew/bin \
     HOME=/tmp/essential-mise-dry-home XDG_CONFIG_HOME=/tmp/essential-mise-dry-xdg \
     XDG_CACHE_HOME=/tmp/essential-mise-dry-cache XDG_DATA_HOME=/tmp/essential-mise-dry-data \
     XDG_STATE_HOME=/tmp/essential-mise-dry-state \
     MISE_CONFIG_DIR=/Users/donbeave/Projects/donbeave/essential-mise \
     MISE_CACHE_DIR=/tmp/essential-mise-dry-cache MISE_DATA_DIR=/tmp/essential-mise-dry-data \
     MISE_STATE_DIR=/tmp/essential-mise-dry-state \
     /tmp/essential-mise-bin/mise -C /tmp -E ai-cli bootstrap --dry-run
   ```

   The partial combined check used the same isolated environment except `MISE_CONFIG_DIR=/tmp/essential-mise-no-conflict`, after copying the repo and deleting only the declaration above:

   ```bash
   /tmp/essential-mise-bin/mise -C /tmp \
     -E foundation,terminal,development,security,js,development-rust,development-java,development-apps,development-containers,development-editors,development-apple,kubernetes,infrastructure,utilities,messengers,browsers,privacy,productivity,office,design,media,network,admin,ai-tools,ai-cli,ai-desktop,system-preferences \
     bootstrap --dry-run
   ```
4. **Isolated Ruby activation:** the first isolated Ruby command resolved to system Ruby 2.6 because the version was not activated. Validation then used the explicit isolated Ruby 4.0.9 binary path, which reported `ruby 4.0.9`.
5. **Temporary trust path:** the first isolated Ruby install failed because its temporary XDG parent did not exist. Creating all temporary directories first fixed it.

## Stage-0 isolated execution

The test used temporary HOME/config directories and local-path adoption of the committed repository; no real global config was touched.

- **First run:** PASS. CLT/Git and Homebrew checks completed, official mise installed into the isolated HOME, local config adopted, foundation wrote `~/.zshenv` and zsh activation, and fresh login+interactive zsh reported `activated: yes`, `shims_on_path: yes`, and `No problems found` in `mise doctor`.
- **Rerun with `env = ["ai-cli", "system-preferences"]`:** PASS. Existing valid checkout skipped adoption and only foundation was reapplied. No optional package was poured. `mise doctor` still reports the selected-but-not-applied packages/defaults as missing, which is expected and not a stage-0 failure.
- **Wrong-origin existing checkout:** PASS. It failed with exit 1 and the explicit refusal to overwrite or pull.
- **Custom `XDG_CONFIG_HOME` workflow:** PASS. Stage 0 adopted into `/tmp/essential-mise-xdg-custom/mise`, activated correctly, persisted `foundation,terminal`, completed `git fetch`/full diff/`pull --ff-only`/dry-run, and completed preview-first `mise bootstrap unapply terminal --dry-run` after removing terminal from the selector. No optional apply was run.
- **Failed launcher download:** PASS. A missing local download returned curl exit 37 and did not execute an empty script.
- **Initial empty-repository test:** the first attempt correctly exposed that no commit existed, so adoption had no config. After the checkpoint commit, the complete flow passed. This was a test-fixture issue, not a bootstrap design change.

6. **Empty local adoption fixture:** the first stage-0 attempt cloned an empty Git repository, so `mise doctor` had no config or activation. Committing the reviewed repository before adoption was required; the rerun passed with full activation.
7. **Optional tools broke stage-0 rerun:** selecting `ai-cli` made unscoped `mise doctor` fail because its optional runtime was intentionally not installed. Stage 0 now runs `mise -E foundation doctor`, matching its foundation-only apply scope. Commit `cc7f478` passed the real optional-selection rerun on the `admin`/`admin` guest.

## Still unverified and requiring disposable-Mac release testing

- Actual installation of remaining non-AI optional modules, including security/network/admin casks and MAS applications, plus existing Homebrew cask conflicts. Amp's unmanaged-app refusal passed.
- Interrupted CLT flows and final review of the immutable release commit. The true no-Homebrew/no-CLT first run passed at `0d57f58` with real Apple dialogs.
- Amp Desktop UI startup on macOS 27.2, currently blocked at `_dyld_start`, plus application launch/relaunch and persistence checks. Ghostty UI, Starship/Nerd Font rendering, zsh/Fish activation, and unrelated-directory checks passed.
- App Store signed-out/signed-in/rerun and `mas` availability. No personal Apple Account is permitted.
- Java/Gradle/Maven active runtime selection and Homebrew OpenJDK coexistence; Rust native Cargo behavior.
- Actual OMP model-role/provider-route loading and authentication, and OMP loading `~/.omp/agent/AGENTS.md`. Native files are applied, but authenticated provider behavior is unverified.
- Login items and live macOS UI behavior beyond the verified stored preferences and nested-dictionary preservation.
- Preview-first unapply and broad package-prune review.
- Second-run convergence for every remaining selected module; standard, AI, and AI Desktop already converged, and stage-0 optional-selection rerun passed.

Static checks do not establish fresh-Mac installation success or full migration parity. Publication is a separate approval and requires the release gate above plus a secret scan, human diff review, and an immutable reviewed commit reference.
