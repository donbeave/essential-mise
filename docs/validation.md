# Validation record

## Environment and safety boundary

Validation uses the official mise **2026.10.23 macOS arm64** binary, downloaded to an isolated path with SHA-256 verification. The checksum was `666b62cf0951b0a8ffccbfa832bfad8e19420d2bd5137b0458ce181f4225e835` before the binary was renamed to `mise` for execution.

Checks use temporary HOME, XDG, cache, data, and state directories. No real-host adoption, package installation, macOS preference change, remote creation, push, credential retrieval, or infrastructure provisioning was performed. Metadata/status commands may inspect current host package state and download only into isolated caches.

The only intentional implementation-host configuration changes were the three approved global Git identity settings.

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
| Full combined `bootstrap --dry-run` | PASS after fixes, with every environment selected |
| Dotfile source references | PASS: all exist; no source symlinks |
| Zed JSONC content | PASS: valid after trailing-comma normalization; no `agent` or `agent_servers` |
| Git INI source | PASS: parses with Python `ConfigParser`; no identity or credential-helper sections |
| Migration inventory | PASS: all 41 components, 54 root dotfiles, and 33 legacy-example dotfiles are individually mapped |
| High-confidence secret scan | PASS: no matches for 1Password references, GitHub tokens, AWS keys, or private-key blocks |
| AI instruction dependency scan | PASS: no RTK/FFF/Caveman/agent-browser/missing-plugin references in approved shared instructions |
| Repeated package ownership audit | PASS: only intentional Nerd Font (terminal/editors), MAS (independent MAS modules), and Ruby (AI tap modules) duplicates |
| Amp Desktop native dry run | PASS after conversion to `macos-app:amp`; official versioned DMG and checksum |
| FFF native dry run | PASS with isolated Ruby 4.0.9: source build `fff-mcp/0.11.0` evaluated |
| Amp CLI/OMP native dry run | PASS with isolated Ruby 4.0.9: both formulas evaluated as source-only builds |
| `claude-code` cask metadata | PASS: Homebrew API returned HTTP 200 |

## Issues found and fixed

1. **Third-party Ruby requirement:** FFF and Amp Desktop initially failed because Ruby 3+ was unavailable. `brew:ruby` was declared first in the two tap-owning formula modules. Isolated Ruby 4.0.9 was then used for validation.
2. **Amp Desktop Ruby DSL incompatibility:** `amp@beta` failed with `undefined method 'second' for an instance of Array`. It was moved to native `macos-app:amp` with version `1.0.994`, the official DMG, and exact SHA-256. Full `ai-desktop` dry run passes. No third-party tap remains in `ai-desktop`.
3. **Existing-host cask conflict:** `ai-cli` dry run stopped at `brew-cask:claude-code: conflicts with installed cask claude-code@latest`. The host was not changed. The remaining `ai-cli` declarations and dotfiles passed separately, and the full combined dry run passed with that existing-host conflict excluded from the isolated test copy. Fresh-machine and ownership-migration testing remains required.
4. **Isolated Ruby activation:** the first isolated Ruby command resolved to system Ruby 2.6 because the version was not activated. Validation then used the explicit isolated Ruby 4.0.9 binary path, which reported `ruby 4.0.9`.
5. **Temporary trust path:** the first isolated Ruby install failed because its temporary XDG parent did not exist. Creating all temporary directories first fixed it.

## Stage-0 isolated execution

Pending final first-run and rerun results. The test uses a temporary HOME/config and local-path adoption of this repository; no real global config is touched. The rerun includes an optional environment selector to prove stage 0 applies only foundation.

## Still unverified and requiring disposable-Mac release testing

- Actual formula/cask/MAS installation, including Amp CLI/Desktop, OMP, FFF, security/network/admin apps, and existing Homebrew ownership behavior.
- CLT absent/interrupted flows and the real public GitHub one-paste command after publication.
- Fresh login/interactive zsh, Fish activation/completions, Starship/Nerd Font rendering, and tools from unrelated directories after persistence.
- App Store signed-out/signed-in/rerun and `mas` availability.
- Java/Gradle/Maven active runtime selection and Homebrew OpenJDK coexistence; Rust native Cargo behavior.
- Actual OMP model-role/provider-route loading and authentication, and OMP loading `~/.omp/agent/AGENTS.md`.
- macOS preference behavior, login items, and preservation of unrelated defaults dictionaries.
- Preview-first unapply and broad package-prune review.
- Second-run convergence for every selected module and safe stage-0 rerun after persistent optional selections.

Static checks do not establish fresh-Mac installation success or full migration parity. Publication is a separate approval and requires the release gate above plus a secret scan, human diff review, and an immutable reviewed commit reference.
