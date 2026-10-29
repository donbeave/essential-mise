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
| Cua Driver package | PENDING on the new `admin`/`admin` automation baseline. The previous candidate verified Homebrew `cuadriver` 0.38.0, Developer ID signature, notarization, and Gatekeeper acceptance; that does not prove the new baseline |
| Guest Homebrew/CLT | IN PROGRESS on the new automation baseline: official Homebrew installer and Command Line Tools for Xcode 27.0 |
| Disposable stage 0 | SUPERSEDED pre-admin evidence: the real commit-pinned launcher, fresh zsh/mise doctor, optional-selection rerun, and wrong-origin refusal passed. Rerun on the new `admin`/`admin` clone with the final configuration |
| Disposable standard modules | SUPERSEDED pre-admin evidence: foundation, terminal, development packages/apps, pinned repositories, copied dotfiles, convergence, and zsh/Fish activation passed. Rerun on the final configuration |
| Guest Cua consent/UI/MCP | **BLOCKED**: CuaDriver did not automatically appear in Accessibility. It must be added manually in the guest using `/Applications/CuaDriver.app` and `admin`/`admin`. No UI or MCP success is claimed |
| Disposable clone deletion | The helper deleted a workflow-owned pre-admin VM and refused unrelated/baseline/unregistered targets. The new baseline/clone lifecycle still requires validation |

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
| Full combined `bootstrap --dry-run` | **PARTIAL — not a complete unmodified preview.** `brew-cask:claude-code` was excluded from the isolated test copy to avoid an existing-host ownership conflict; every other declaration and the combined config passed |
| Dotfile source references | PASS: all exist; no source symlinks |
| Zed JSONC content | PASS: valid after trailing-comma normalization; no `agent` or `agent_servers` |
| Git INI source | PASS: parses with Python `ConfigParser`; no identity or credential-helper sections |
| Migration inventory | PASS: all 41 components, 54 root dotfiles, and 33 legacy-example dotfiles are individually mapped |
| High-confidence secret scan | PASS: no matches for 1Password references, GitHub tokens, AWS keys, or private-key blocks |
| AI instruction dependency scan | PASS: no RTK/FFF/Caveman/agent-browser/missing-plugin references in approved shared instructions |
| Repeated package ownership audit | PASS: intentional Nerd Font, independent MAS, and shared `[tools]` Ruby declarations; no `http:omp`, native `macos-app:amp`, or second owner remains |
| Amp Desktop ownership declaration | PASS: real-Homebrew `ampcode/tap/amp@beta` hook with an unmanaged-app refusal; no `macos-app` or adoption declaration. Clean/conflict runtime tests remain |
| FFF native dry run | PASS with isolated Ruby 4.0.9: source build `fff-mcp/0.11.0` evaluated; actual installation remains |
| Amp CLI actual source build | PASS on disposable macOS 27.2: `ampcode` 0.0.1793304039-g62d85d built and installed natively. OMP's native build failed and is now a documented real-Homebrew exception |
| Native Ruby prerequisite | PASS on disposable macOS 27.2: `[tools] ruby = "4.0.9"` enabled the actual third-party tap preview. The earlier `brew:ruby` approach installed `/opt/homebrew/bin/ruby` but did not fix mise's evaluation environment |
| `claude-code` cask metadata | PASS: Homebrew API returned HTTP 200 |

## Issues found and fixed

1. **Third-party Ruby requirement:** the original preview without Ruby 3+ failed. Installing `brew:ruby` did not repair subsequent mise tap evaluation even though a fresh shell found Ruby 4.0.9. Both tap-owning AI modules now declare `[tools] ruby = "4.0.9"`; `mise -E ai-tools,ai-cli install ruby` is the native prerequisite, and the actual tap preview passed. A clean final apply/convergence run remains required.
2. **Real-Homebrew compatibility exceptions:** Amp Desktop's cask failed with `undefined method 'second' for an instance of Array`; OMP's formula failed with `formula uses with_env (install-time helper), which mise's source-build shim does not support`. Their native declarations were removed. Documented real-Homebrew hooks now own Amp Desktop and OMP, with an existing-Amp guard and no automatic adoption. See [Homebrew exceptions](homebrew-exceptions.md). The revised hooks require clean `admin`/`admin` VM verification.
3. **Existing-host cask conflict / partial combined preview:** the unmodified `ai-cli` dry run stopped at `brew-cask:claude-code: conflicts with installed cask claude-code@latest`. The host was not changed. The isolated test copy removed exactly `"brew-cask:claude-code" = "latest"`; that filtered copy and the remaining combined declarations passed. This is **PARTIAL**, not a complete preview. A complete unmodified dry run and an existing-Homebrew ownership test remain release requirements.

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

## Still unverified and requiring disposable-Mac release testing

- Actual formula/cask/MAS installation, including Amp CLI/Desktop, OMP, FFF, security/network/admin apps, and existing Homebrew ownership behavior.
- Complete unmodified full-combination dry run, plus existing Homebrew cask conflict handling on a separate Mac.
- First-time AI preview, apply, and status on macOS 27.2 without a separately provisioned Ruby 3+, including the native two-stage recovery and second-run convergence.
- CLT absent/interrupted flows and the real public GitHub one-paste command after publication.
- Fresh login/interactive zsh, Fish activation/completions, Starship/Nerd Font rendering, and tools from unrelated directories after persistence.
- App Store signed-out/signed-in/rerun and `mas` availability.
- Java/Gradle/Maven active runtime selection and Homebrew OpenJDK coexistence; Rust native Cargo behavior.
- Actual OMP model-role/provider-route loading and authentication, and OMP loading `~/.omp/agent/AGENTS.md`.
- macOS preference behavior, login items, and preservation of unrelated defaults dictionaries.
- Preview-first unapply and broad package-prune review.
- Second-run convergence for every selected module and safe stage-0 rerun after persistent optional selections.

Static checks do not establish fresh-Mac installation success or full migration parity. Publication is a separate approval and requires the release gate above plus a secret scan, human diff review, and an immutable reviewed commit reference.
