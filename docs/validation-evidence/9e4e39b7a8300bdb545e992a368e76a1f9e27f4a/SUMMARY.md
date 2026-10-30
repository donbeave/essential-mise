# Shell, XDG, and validation-harness review fixes — `9e4e39b`

**Status: PRE-RELEASE.** This bundle records real disposable-guest validation, not unrestricted public-release approval. Earlier bundles remain useful historical evidence; this bundle supersedes their fixed-path Fish/GitHub CLI behavior and incomplete linked-worktree harness coverage.

## Source and environment

- Configuration and fixture revision: `9e4e39b7a8300bdb545e992a368e76a1f9e27f4a`.
- Guests: `essential-mise-test-release` and `essential-mise-test-audit`, macOS 27.2 (26B123), arm64, 4 CPUs, 8192 MB RAM, 100 GB disk.
- mise: 2026.10.23 macos-arm64 (2026-10-29); Tart 2.41.1; synthetic `admin`/`admin` accounts only.
- Both preserved baselines and both test guests were stopped after collection. No application under test ran on the physical host, no host directory was shared, and no personal credentials or accounts were used.

## Verified results

| Area | Evidence and observed result |
|---|---|
| Standard modules | `standard/` records package/setting application, repeat convergence, zsh/Fish activation, GitHub CLI smoke readbacks, and shell verification at the reviewed SHA. |
| AI modules | `ai/` records installation/status, repeat convergence, and CLI health checks. The alias-safe Claude declaration and Fish Amp block are applied. |
| Actual custom-XDG applications | `xdg-applications/report.txt` passes all checks. GitHub CLI consumes every managed scalar, alias, and schema value at custom XDG and `GH_CONFIG_DIR`; Ghostty `+show-config`/`+validate-config` consumes its config and custom theme; Fish executes mise activation and Amp completions. No default-path copies are used. |
| XDG preservation and removal | Repeated XDG application converges. Unrelated GitHub CLI/Fish content survives. Native unapply removes custom-XDG Ghostty files; explicit helper unapply removes only Fish blocks. |
| Deterministic isolated lifecycle | `isolated-fixtures/report.txt` starts with PASS and contains 49 expected command-result rows. Exact HEAD identity, a controlled-ref fast-forward update, custom-XDG Ghostty deployment, real Fish activation, fresh zsh selection, native unapply, and Fish-block removal all pass. |
| Linked-worktree attribution | `linked-worktree-wrong-revision/` and `linked-worktree-dirty/` record the actual SHA/status and fail before apply with host exit 1. `linked-worktree-full-dry-run/` records a clean `.git`-file worktree at the reviewed SHA and completes. `commands/` preserves the setup and guard results. |
| Claude wrapper | `claude-wrapper/report.txt` passes existing-alias coexistence, later startup execution, alias-argument forwarding, special-character XDG quoting, JSON encoding, and synthetic rendering. The status-line script hash remains `e0268e5e61bb07456bdbbcda5975c7189f5e89168b8cd3df6821cadc2c05917d`. |
| Git and Zed preservation | `configuration-safety/` preserves Git SSH settings, repeated helpers, `includeIf`, comments, and mixed indentation; custom-XDG ignore works; Zed JSONC preserves comments, trailing commas, and unrelated keys. |
| Presence-only Homebrew hook | `hook-lifecycle/` keeps Amp CLI `0.0.1793352886-g7ad87b` unchanged with no formula/link changes and a passing health check. Structured `outdated.json`, livecheck, and current formula metadata establish the outdated state; upstream discovery alone is not the proof. |
| Terminal removal and full preview | `unapply/` records native terminal removal, explicit Fish-block removal, and reapplication. `full-dry-run/` resolves all 27 environments without exclusions; this is not simultaneous-install evidence. |
| Current inventory and mutations | `static-checks.txt` records 129 packages, five tools, 17 dotfile targets, one setting, 67 preferences, two repositories, two login items, and two executables. `inventory-mutations.txt` proves missing owner-specific jq, unknown login paths, and a missing Fish helper record fail closed. |

Intermediate runs exposed and corrected an unsupported hook-template variable, an alias-aware assertion, and a missing Fish test prerequisite. This bundle contains only complete final-candidate runs; no earlier failure is represented as PASS, and no stale successful artifact closes a failed run.

## Remaining verification and accepted boundaries

This bundle does not establish every module's independent clean installation, an applied native package/cask upgrade, native-cask ownership during an actual upgrade/prune, a real new-Terminal UI session, or an immediate preference snapshot/restore cycle. Those remain open validation gates, not successful claims.

The scoped-v1 decisions are unchanged: signed-in installation and reruns for 21 MAS applications are manual/unverified; Amp Desktop's macOS 27.2 GUI failure is accepted; authenticated OMP/Claude behavior and application onboarding remain manual; complete preference UI behavior and delayed selective reversal remain manual. The repository stays PRE-RELEASE.

## Integrity and sanitization

Terminal control sequences were removed except from synthetic status-line color evidence. Exit records, fixture hashes, high-confidence secret patterns, and source identities were checked. `SHA256SUMS` covers 398 files in this directory; only the checksum manifest itself is excluded.
