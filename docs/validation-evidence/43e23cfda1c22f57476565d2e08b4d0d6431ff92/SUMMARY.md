# Validation evidence — `43e23cfda1c22f57476565d2e08b4d0d6431ff92`

**Status: PRE-RELEASE.** These artifacts record guest tests; they are not a blanket release approval.

## Environment

- Source: `https://github.com/donbeave/essential-mise/commit/43e23cfda1c22f57476565d2e08b4d0d6431ff92`
- Host: arm64, macOS 27.2 (26B123); Tart 2.41.1
- Guests: `essential-mise-test-release` and `essential-mise-test-final`, macOS 27.2 (26B123), 4 CPUs, 8192 MB RAM, 100 GB disk; exact Tart configurations are in `vm-release.json` and `vm-final.json`.
- Preserved baselines: `essential-mise-fresh-baseline` and `essential-mise-automation-baseline`.
- mise: 2026.10.23 macos-arm64 (2026-10-29); Cua Driver: 0.38.0.
- Only synthetic `admin`/`admin` guest accounts were used. No application under test ran on the physical host, no host directory was shared, and no personal account or credential was used.

## Evidence index

- `release/`: standard and AI modules, optional non-MAS installation and convergence, Java/Rust activation, terminal unapply/reapply, reviewed prune candidates, preferences, login items, Surge ownership refusal/restoration, OMP configuration, and the unmodified 27-environment preview.
- `final/`: clean public stage 0, interrupted-CLT recovery, standard and AI installation, Java/Rust, Claude ownership, OMP configuration, status-line fixtures, Cua accessibility/screenshots, and MCP over SSH.
- `isolated-fixtures/`: exact first-run, optional-selection rerun, wrong-origin, and alternate-XDG commands, logs, revisions, mise versions, and exit codes.
- `amp-startup/`: official DMG identity, signature/Gatekeeper checks, no-window assertion, process state, and the `_dyld_start` sample.
- `amp-conflict/`: the unmanaged-app marker remains unchanged, Homebrew has no ownership record, and the exact ownership-refusal message is asserted.

## Command exit codes

| Artifact | Exit | Result |
|---|---:|---|
| `amp-conflict/existing-app-refusal.exit` | 1 | EXPECTED REFUSAL |
| `final/ai/apply.exit` | 0 | PASS |
| `final/ai/cli-verification.exit` | 0 | PASS |
| `final/ai/preview.exit` | 0 | PASS |
| `final/ai/second-run.exit` | 0 | PASS |
| `final/ai/status.exit` | 0 | PASS |
| `final/ai-desktop/apply.exit` | 0 | PASS |
| `final/ai-desktop/preview.exit` | 0 | PASS |
| `final/ai-desktop/second-run.exit` | 0 | PASS |
| `final/ai-desktop/verification.exit` | 0 | PASS |
| `final/claude-conflict/cli-verification.exit` | 0 | PASS |
| `final/claude-conflict/existing-apply.exit` | 0 | PASS |
| `final/claude-conflict/existing-preview.exit` | 0 | PASS |
| `final/claude-conflict/install-existing.exit` | 0 | PASS |
| `final/claude-conflict/second-run.exit` | 0 | PASS |
| `final/claude-statusline/output-35.exit` | 0 | PASS |
| `final/claude-statusline/output-50.exit` | 0 | PASS |
| `final/claude-statusline/output-80.exit` | 0 | PASS |
| `final/full-dry-run/all-environments.exit` | 0 | PASS |
| `final/interrupted-clt/first-run.exit` | 1 | EXPECTED REFUSAL |
| `final/java/apply.exit` | 0 | PASS |
| `final/java/build-tools.exit` | 0 | PASS |
| `final/java/exec-check.exit` | 0 | PASS |
| `final/java/persistent.exit` | 0 | PASS |
| `final/java/preview.exit` | 0 | PASS |
| `final/omp-config/config-scope.exit` | 0 | PASS |
| `final/omp-config/config.exit` | 0 | PASS |
| `final/omp-config/instructions.exit` | 0 | PASS |
| `final/omp-config/model-roles.exit` | 0 | PASS |
| `final/omp-config/models.exit` | 0 | PASS |
| `final/omp-config/unauthenticated-check.exit` | 0 | PASS |
| `final/rust/apply.exit` | 0 | PASS |
| `final/rust/build-check.exit` | 0 | PASS |
| `final/rust/exec-check.exit` | 0 | PASS |
| `final/rust/persistent.exit` | 0 | PASS |
| `final/rust/preview.exit` | 0 | PASS |
| `final/rust/tool-check.exit` | 0 | PASS |
| `final/stage0/first-run.exit` | 0 | PASS |
| `final/stage0/optional-selection-rerun.exit` | 0 | PASS |
| `final/stage0/shell-verification.exit` | 0 | PASS |
| `final/stage0/wrong-origin.exit` | 1 | EXPECTED REFUSAL |
| `final/standard/apply.exit` | 0 | PASS |
| `final/standard/preview.exit` | 0 | PASS |
| `final/standard/second-run.exit` | 0 | PASS |
| `final/standard/shell-verification.exit` | 0 | PASS |
| `final/standard/status.exit` | 0 | PASS |
| `release/ai/apply.exit` | 0 | PASS |
| `release/ai/cli-verification.exit` | 0 | PASS |
| `release/ai/preview.exit` | 0 | PASS |
| `release/ai/second-run.exit` | 0 | PASS |
| `release/ai/status.exit` | 0 | PASS |
| `release/ai-desktop/apply.exit` | 0 | PASS |
| `release/ai-desktop/preview.exit` | 0 | PASS |
| `release/ai-desktop/second-run.exit` | 0 | PASS |
| `release/ai-desktop/verification.exit` | 0 | PASS |
| `release/claude-conflict/cli-verification.exit` | 0 | PASS |
| `release/claude-conflict/existing-apply.exit` | 0 | PASS |
| `release/claude-conflict/existing-preview.exit` | 0 | PASS |
| `release/claude-conflict/install-existing.exit` | 0 | PASS |
| `release/claude-conflict/second-run.exit` | 0 | PASS |
| `release/claude-statusline/output-35.exit` | 0 | PASS |
| `release/claude-statusline/output-50.exit` | 0 | PASS |
| `release/claude-statusline/output-80.exit` | 0 | PASS |
| `release/full-dry-run/all-environments.exit` | 0 | PASS |
| `release/java/apply.exit` | 0 | PASS |
| `release/java/build-tools.exit` | 0 | PASS |
| `release/java/exec-check.exit` | 0 | PASS |
| `release/java/persistent.exit` | 0 | PASS |
| `release/java/preview.exit` | 0 | PASS |
| `release/login-items/apply.exit` | 0 | PASS |
| `release/login-items/preview.exit` | 0 | PASS |
| `release/login-items/status.exit` | 0 | PASS |
| `release/login-items/verify.exit` | 0 | PASS |
| `release/mas-signed-out/cli.exit` | 0 | PASS |
| `release/mas-signed-out/config.exit` | 0 | PASS |
| `release/mas-signed-out/metadata.exit` | 0 | PASS |
| `release/mas-signed-out/verify-not-installed.exit` | 0 | PASS |
| `release/omp-config/config-scope.exit` | 0 | PASS |
| `release/omp-config/config.exit` | 0 | PASS |
| `release/omp-config/instructions.exit` | 0 | PASS |
| `release/omp-config/model-roles.exit` | 0 | PASS |
| `release/omp-config/models.exit` | 0 | PASS |
| `release/omp-config/unauthenticated-check.exit` | 0 | PASS |
| `release/optional-modules/apply.exit` | 0 | PASS |
| `release/optional-modules/cli-verification.exit` | 0 | PASS |
| `release/optional-modules/package-verification.exit` | 0 | PASS |
| `release/optional-modules/preview.exit` | 0 | PASS |
| `release/optional-modules/second-run.exit` | 0 | PASS |
| `release/optional-modules/status-json.exit` | 0 | PASS |
| `release/optional-modules/status.exit` | 0 | PASS |
| `release/preferences/apply.exit` | 0 | PASS |
| `release/preferences/behavior-verification.exit` | 0 | PASS |
| `release/preferences/preview.exit` | 0 | PASS |
| `release/preferences/stored-status.exit` | 0 | PASS |
| `release/prune-review/candidates.exit` | 0 | PASS |
| `release/prune-review/install-unrelated.exit` | 0 | PASS |
| `release/prune-review/manual-cleanup.exit` | 0 | PASS |
| `release/rust/apply.exit` | 0 | PASS |
| `release/rust/build-check.exit` | 0 | PASS |
| `release/rust/exec-check.exit` | 0 | PASS |
| `release/rust/persistent.exit` | 0 | PASS |
| `release/rust/preview.exit` | 0 | PASS |
| `release/rust/tool-check.exit` | 0 | PASS |
| `release/stage0/first-run.exit` | 0 | PASS |
| `release/stage0/optional-selection-rerun.exit` | 0 | PASS |
| `release/stage0/shell-verification.exit` | 0 | PASS |
| `release/stage0/wrong-origin.exit` | 1 | EXPECTED REFUSAL |
| `release/standard/apply.exit` | 0 | PASS |
| `release/standard/preview.exit` | 0 | PASS |
| `release/standard/second-run.exit` | 0 | PASS |
| `release/standard/shell-verification.exit` | 0 | PASS |
| `release/standard/status.exit` | 0 | PASS |
| `release/surge-conflict/existing-app-refusal.exit` | 1 | EXPECTED REFUSAL |
| `release/surge-conflict/restore.exit` | 0 | PASS |
| `release/surge-conflict/uninstall-existing.exit` | 0 | PASS |
| `release/surge-conflict/verify-owner.exit` | 0 | PASS |
| `release/surge-conflict/verify-restored.exit` | 0 | PASS |
| `release/unapply/reapply.exit` | 0 | PASS |
| `release/unapply/unapply.exit` | 0 | PASS |
| `release/unapply/verify-removed.exit` | 0 | PASS |
| `release/unapply/verify-restored.exit` | 0 | PASS |

The full 27-environment dry run proves unmodified preview resolution in the recorded guest state, not simultaneous installation of every environment. All 35 non-MAS optional brew-cask declarations and eight non-MAS optional brew declarations were installed; the 21 MAS applications remain missing without an App Store account.

## Open boundaries

- MAS signed-out behavior was observed and the sign-in dialog was cancelled. Signed-in installation, purchases, and rerun behavior remain BLOCKED without a synthetic Apple Account.
- Amp Desktop `1.0,998` is installed and signed, but its main thread remains stalled at `_dyld_start` with no window. GUI setup is MANUAL/DROP on macOS 27.2.
- OMP loads the declared model roles. `omp models` reports no available models without provider credentials; authenticated models, provider routes, and instruction loading at runtime remain UNVERIFIED.
- Claude status-line fixtures and `--settings` merging passed. Authenticated interactive rendering remains UNVERIFIED.
- Account sign-in, licenses, privacy approvals, privileged application setup, and app-specific onboarding remain manual where documented.

## Sanitization and integrity

App bundles, symlinked artifacts, VNC endpoint logs, and personal data are excluded. Text logs have terminal control sequences removed and high-confidence credential patterns redacted; status-line outputs retain their synthetic ANSI color evidence. Plists are converted to XML. Screenshots originate from the guest or Tart's guest framebuffer, never the host desktop. `SHA256SUMS` covers every file in this directory except itself.
