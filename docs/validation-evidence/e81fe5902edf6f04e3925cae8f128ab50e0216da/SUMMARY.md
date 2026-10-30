# Focused review-fix validation — `e81fe5902edf6f04e3925cae8f128ab50e0216da`

**Status: PRE-RELEASE.** These artifacts add focused regression evidence to the complete bundle at `../43e23cfda1c22f57476565d2e08b4d0d6431ff92/`; they are not a replacement for that bundle or a blanket release approval.

## Environment and scope

- Source: `https://github.com/donbeave/essential-mise/commit/e81fe5902edf6f04e3925cae8f128ab50e0216da`
- Host: arm64, macOS 27.2 (26B123); Tart 2.41.1.
- Guest: `essential-mise-test-release`, macOS 27.2 (26B123), 4 CPUs, 8192 MB RAM, 100 GB disk.
- mise: 2026.10.23 macos-arm64 (2026-10-29).
- Only the synthetic `admin`/`admin` guest account was used. No application under test ran on the physical host, no host directory was shared, and no personal account or credential was used.
- `release/ai/` records preview, real apply, repeat convergence, status, and CLI health checks at the exact source revision.
- `release/java/` records the fail-closed persistent Java assertion and Maven/Gradle checks after persistent GraalVM activation.
- `release/claude-wrapper/` records the rendered zsh function, a mock `claude` argv capture, decoded settings, and actual execution of the resulting status-line command through `/bin/sh`.
- `release/claude-statusline/` records the unchanged script's exact sample, context-color thresholds, subagent filtering, and input-token formula.

## Command exit codes

| Artifact | Exit | Result |
|---|---:|---|
| `release/ai/preview.exit` | 0 | PASS |
| `release/ai/apply.exit` | 0 | PASS |
| `release/ai/status.exit` | 0 | PASS |
| `release/ai/second-run.exit` | 0 | PASS |
| `release/ai/cli-verification.exit` | 0 | PASS |
| `release/java/preview.exit` | 0 | PASS |
| `release/java/apply.exit` | 0 | PASS |
| `release/java/exec-check.exit` | 0 | PASS |
| `release/java/build-tools.exit` | 0 | PASS |
| `release/java/persistent.exit` | 0 | PASS |
| `release/java/persistent-build-tools.exit` | 0 | PASS |
| `release/claude-wrapper/wrapper.exit` | 0 | PASS |
| `release/claude-wrapper/rendered.exit` | 0 | PASS |
| `release/claude-statusline/output-35.exit` | 0 | PASS |
| `release/claude-statusline/output-50.exit` | 0 | PASS |
| `release/claude-statusline/output-80.exit` | 0 | PASS |

## Observed corrections

- The optional Homebrew OpenJDK query in `persistent` has its own braces and `|| true`; it no longer masks an earlier Java version or runtime-path failure.
- Fresh-login Maven 3.10.0 and Gradle 9.8.2 report Oracle Corporation Java 25.0.4.1.1 under `/Users/admin/.local/share/mise/installs/java/oracle-graalvm-25.0.4.1.1`. The pre-persistence `build-tools` log intentionally documents the prior Homebrew OpenJDK selection rather than mislabeling it as GraalVM.
- The Claude fixture uses an XDG path containing spaces, single and double quotes, a dollar sign, and a backslash. It verifies zsh shell quoting, JSON encoding, unchanged argument forwarding, and successful execution of the emitted command.
- The status-line script remains byte-for-byte unchanged at SHA-256 `e0268e5e61bb07456bdbbcda5975c7189f5e89168b8cd3df6821cadc2c05917d`.

## Open boundaries

This focused run does not change the release blockers in the main validation record: signed-in MAS installation/reruns, Amp Desktop's confirmed `_dyld_start` GUI failure, authenticated OMP routing, and authenticated interactive Claude rendering. Prune behavior is documented from the prior complete evidence bundle; no broad prune was executed in this run.

## Sanitization and integrity

Terminal control sequences were removed except from synthetic status-line color evidence. App bundles, symlinks, host screenshots, personal data, and credentials are excluded. High-confidence private-key/token scans passed. `SHA256SUMS` covers every file in this directory except itself.
