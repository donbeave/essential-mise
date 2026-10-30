# Validation record

**Status: PRE-RELEASE. The disposable-Mac release gate has not passed.** The public `main` branch is not an end-to-end-tested release source. Use a reviewed commit-pinned launcher for any real setup.

## Environment and evidence boundary

Validation targets Apple Silicon macOS **27.2 (26B123)**, mise **2026.10.23 macos-arm64**, Tart **2.41.1**, and Cua Driver **0.38.0**. The host has 64 GB RAM. Test guests have 4 CPUs, 8192 MB RAM, and a 100 GB disk.

`essential-mise-fresh-baseline` and `essential-mise-automation-baseline` are stopped and preserved. Disposable guests include `essential-mise-test-release`, `essential-mise-test-final`, `essential-mise-test-amp-startup`, and `essential-mise-test-amp-conflict`. All guest accounts and data are synthetic. No application under test runs on the physical host, no host directory is shared, clipboard sharing is disabled, and NAT is not treated as complete security isolation. SIP, Gatekeeper, and normal TCC behavior remain enabled.

Exact source revisions, fixture checksums, mise versions, guest OS/build values, commands, logs, exit codes, screenshots, and artifact hashes are preserved in [validation evidence](validation-evidence/). Each evidence directory contains a `SUMMARY.md` and `SHA256SUMS`. Temporary guest evidence is collected before rebooting or stopping the guest; `/tmp` is not treated as durable evidence storage.

## Verified on disposable macOS 27.2

| Area | Verified result |
|---|---|
| Cold stage 0 | A guest with neither Homebrew nor Command Line Tools completed Apple's install/license flow, Homebrew installation, pinned mise installation, public adoption, foundation activation, and fresh-shell doctor. The interrupted first attempt refused to continue before CLT was ready; the rerun succeeded. Native mise confirmations are handled explicitly only in the test helper. |
| Stage-0 rerun safety | A persisted optional selection does not make the launcher install optional packages. Existing checkouts skip adoption and apply only foundation. An unexpected origin is refused. |
| Isolated fixtures | First-run, optional-selection rerun, wrong-origin, and alternate-XDG workflows pass with exact exit codes and local-path adoption substituted only in a temporary launcher copy. See [Replayable fixtures](validation-fixtures.md). |
| Standard modules | Foundation, terminal, and development packages/apps, copied dotfiles, pinned Oh My Zsh and zsh-autosuggestions revisions, repeat convergence, and zsh/Fish activation pass. Ghostty's app-bundle executable is checked directly. |
| AI CLI and shared tools | Preview, actual installation, status, repeat convergence, and CLI checks pass. This includes `rg`, `fd`, `rtk`, `agent-browser`, Claude Code, Codex, Kimi, OpenCode, Amp CLI, OMP, and FFF. `amp --version`, `omp --version`, and `fff-mcp --healthcheck` pass. |
| AI desktop packages | Claude, ChatGPT, Cursor, Antigravity, Kimi, OpenCode Desktop, ZCode, CodexBar, Grok Bot, and Amp Desktop install and converge. Package installation is not a claim that every GUI is usable or authenticated. |
| Optional non-MAS modules | All 35 optional `brew-cask` declarations and eight optional `brew` declarations report installed. Repeat application converges. Node `26.9.0`, Bun `1.4.4`, Kubernetes, Helm, OpenTofu, Ansible, linode-cli, gomplate, and 1Password CLI execute successfully. Privileged pkg casks use a real guest TTY; a remote output pipe was a fixture error, not a reason for another Homebrew exception. |
| Java | GraalVM `25.0.4.1.1`, Maven `3.10.0`, Gradle `9.8.2`, and Homebrew OpenJDK coexist. One-off and persistent runtime selection pass. |
| Rust and mr-boxington | Rust/Cargo `1.99.1`, rustc, native Cargo command-wrapper selection, and persistent activation pass. A real `cargo check` smoke build succeeds in both test guests and records mr-boxington cache activity. The unsupported explicit wrapper was removed. |
| Existing Claude Code ownership | An existing real-Homebrew `claude-code@latest` installation is reused, with successful preview, apply, repeat, and `claude --version`. No second owner or automatic adoption is introduced. |
| Amp ownership | The real-Homebrew `amp@beta` hook refuses an unmanaged `/Applications/Amp.app`. The marker remains unchanged, Homebrew has no ownership record, and the exact refusal reason is asserted. |
| Surge ownership | Native mise cannot expand the cask binary's literal `$APPDIR` source. Real Homebrew installs and links Surge correctly. The hook refuses an unmanaged app without changing its marker, then cleanly restores the Homebrew-owned installation after explicit test cleanup. |
| Terminal removal | Actual `mise bootstrap unapply terminal --yes` removes managed files/edits; reapplication restores them. This is not merely an empty unapply preview. |
| Prune review | The dry run identifies unrelated `sl` as `brew:sl@...` and also identifies working hook-installed Amp CLI, FFF, and OMP as candidates. The unrelated fixture is removed explicitly with Homebrew. No automatic broad prune runs; broad native formula pruning is prohibited while these tools must remain installed. |
| macOS preferences | Preview, apply, stored status, Dock values, and preservation of unrelated global/nested values pass, including `AppleFirstWeekday.syntheticKeep` and unrelated `AppleICUDateFormatStrings` members. Stored-value matching is not treated as proof of every UI behavior. |
| Login items | CleanShot X and Bartender are registered and actually launch after a real guest reboot. CleanShot's license prompt remains a manual decision; the application is not left configured or licensed. |
| Claude status line | The unchanged script produces the exact requested sample, hides finished/interrupted/idle subagents, and passes green/yellow/red thresholds. Its SHA-256 is `e0268e5e61bb07456bdbbcda5975c7189f5e89168b8cd3df6821cadc2c05917d`. The wrapper forwards arguments, preserves observed project settings, requests a five-second refresh, and uses the documented input-token context formula. |
| OMP configuration | `omp config --scope global --json` reports model roles identical to the retained YAML. No provider credential is present, so authenticated routing and instruction loading at runtime are not claimed. |
| Cua automation | Accessibility, Screen Recording, and direct-capture consent are granted normally inside the guest under CuaDriver's own identity. Calculator 6×7 is verified as 42 through accessibility and a screenshot. MCP over SSH exposes 62 tools and captures only the guest desktop. |
| Complete preview | All 27 environments complete an unmodified `mise bootstrap --dry-run` with no excluded declarations. This proves preview resolution in the recorded guest state, not simultaneous installation of every environment. |

The complete migration matrix retains 41 component rows, 54 root-dotfile rows, and 33 legacy-example-dotfile rows. Those counts establish inventory coverage, not behavioral parity. Git INI and Zed JSONC parsing, explicit copy-mode dotfiles, source existence, and the no-secret/high-confidence scans also pass.

## Compatibility exceptions and management limits

Native mise remains the default. Five AI package exceptions were explicitly approved; Surge is the sixth under the user's conditional real-Homebrew fallback for demonstrated current-mise failures. Exact errors and evidence are in [Homebrew exceptions](homebrew-exceptions.md).

These hooks are not declarative package resources. `mise bootstrap status --missing`, native package upgrades, and native prune must not be relied on to manage hook-installed packages. The observed prune dry run offered to remove working Amp CLI, FFF, and OMP hook installations; applying a broad native formula prune is prohibited while they must remain installed. Review candidates and perform separately approved, explicit Homebrew cleanup instead. Each hook runs health checks on every apply. Upgrade and removal owners are documented individually. Deselecting a module does not uninstall these packages, and no automatic ownership transfer is allowed.

The Claude status-line script is a separate approved exception and remains byte-for-byte unchanged. Its displayed absolute usage is input-token usage, matching the documented `used_percentage` formula; output tokens are intentionally not added.

## Remaining release blockers

- **App Store:** 21 MAS applications are still missing, including Apple Configurator (`mas:1037126344`). Signed-out metadata and the sign-in/cancel flow are verified, but signed-in installation, purchases/claims, and rerun behavior are blocked without a synthetic Apple Account. Personal accounts are not permitted.
- **Amp Desktop GUI:** `1.0,998` installs with a valid signature and an executable byte-identical to the official DMG. Independent guests reproduce a main-thread `_dyld_start` stall with no window. GUI setup is **MANUAL / DROP** on macOS 27.2; no signing or security bypass is introduced.
- **Authenticated AI behavior:** OMP provider routes, authenticated models, and runtime instruction loading are unverified. Authenticated interactive Claude status-line rendering is also unverified.
- **Application onboarding:** accounts, licenses, paid purchases, privacy consents, privileged extensions, browser/VPN profiles, and app-specific setup remain manual as documented.

The repository must remain PRE-RELEASE until these boundaries are resolved or explicitly accepted. A successful package installation, dry run, CLI version, or screenshot alone is not release approval.
