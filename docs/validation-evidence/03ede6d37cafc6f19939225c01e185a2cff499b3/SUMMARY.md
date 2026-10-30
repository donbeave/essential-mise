# Configuration-safety and cold-install validation — `03ede6d`

**Status: PRE-RELEASE.** This bundle supplements the earlier complete and focused bundles. It records real guest tests, not blanket public-release approval. Accepted MAS, Amp GUI, authenticated-AI, and preference behavior boundaries remain visible in `docs/validation.md`.

## Environment and source identity

- Bundle source: `03ede6d37cafc6f19939225c01e185a2cff499b3`; scenario-specific source revisions are recorded in `source-revisions.txt` and each `scenario.env`.
- Host: arm64 macOS 27.2 (26B123); Tart 2.41.1; 64 GB RAM.
- Guests: `essential-mise-test-cold`, `essential-mise-test-audit`, `essential-mise-test-release`, and `essential-mise-test-cask2`; macOS 27.2 (26B123), 4 CPUs, 8192 MB RAM, 100 GB disk.
- mise: 2026.10.23 macos-arm64 (2026-10-29).
- Preserved baselines: `essential-mise-fresh-baseline` and `essential-mise-automation-baseline`, stopped. All test guests were stopped after collection.
- Only synthetic `admin`/`admin` guest accounts and synthetic files were used. No application under test ran on the physical host, no host directory was shared, and no personal account or credential was used.

## Evidence index and observed results

| Area | Artifact and result |
|---|---|
| Interrupted CLT | `interrupted-clt/cold-first-run.exit` is 1. The guest refused to continue before CLT was ready. `commands/essential-mise-cold-clt.log` records the subsequent exact guest-only Software Update installation. |
| Successful cold stage 0 | `cold-stage0/cold-preconditions.log` records no Homebrew/config. `cold-first-run.log` contains the real Homebrew installer and first public adoption, not a rerun. The clean adopted SHA is `94890d714202da5c3116a862264c6f421201f007`; `adopted-mise-version.txt` records 2026.10.23. Foundation and fresh-shell checks pass. |
| Existing Homebrew and checkout guards | `isolated-fixtures/` records successful empty `shellenv` startup, wrong-origin/HOME/root/symlink/bare/subdirectory refusals, no adoption through the symlink, and a valid linked worktree. |
| Complete XDG lifecycle | All 42 command-result rows in `isolated-fixtures/report.txt` have expected outcomes, including adopted revision checks, a real reset/nonempty diff/fast-forward update, dotfile apply, unrelated fresh shell, and actual unapply/removal. The legacy report retains its initial IN_PROGRESS status despite its successful completion message; the `9e4e39b…` fixture supersedes it with exact HEAD checks and an explicit 49-row PASS. |
| Standard modules | `standard/` records initial installation, repeat convergence, zsh/Fish activation, GitHub CLI scalar/alias/schema readbacks, and absence of unsafe Git merge/copy targets. |
| AI and independence | `ai/` records installation, status, repeat convergence, and health checks. `ai-cli-independent/` records `ai-cli` working without `ai-tools`, including jq/Claude/Amp/OMP and no FFF executable. |
| Git and merge preservation | `configuration-safety/` proves repeated Git CLI setup preserves user SSH settings, helpers, includeIf, comments, and mixed indentation; custom-XDG ignore works; native Zed JSONC and GitHub CLI YAML preserve unrelated user content. |
| Presence-only hooks | `hook-lifecycle/` proves Amp CLI `0.0.1793352886-g7ad87b` remains installed despite livecheck offering `0.0.1793363705-g5c9ae5`; formula inventory and link are unchanged and the health check passes. |
| JavaScript | `js/` records actual Node 26.9.0/Bun 1.4.4 installation, one-off execution, and persistent fresh-shell path selection. |
| Native cask coexistence | `native-cask-coexistence/` records a real-Homebrew Ghostty installation preserved through native terminal apply and second-run convergence. |
| Amp ownership refusal | `amp-conflict/existing-app-refusal.exit` is the expected 1. The unmanaged marker remains, no Homebrew owner exists, and the exact refusal text is asserted. |
| Project/workspace boundaries | `project-boundaries/` records `.miserc`/local/CLI precedence, project resource/environment isolation, and ignored synthetic workspace activation using explicitly trusted synthetic files. |
| Native upgrade preview | `native-upgrade-preview/upgrade.exit` is 0. This validates the documented `mise bootstrap --upgrade --dry-run` workflow without applying upgrades. |
| Complete preview | `full-dry-run/all-environments.exit` is 0 for all 27 environments with no exclusions. This is preview resolution, not simultaneous-install proof. |
| Failure freshness | `isolated-failure-lifecycle/report.txt` contains FAIL. `statusline-lifecycle/` contains the deliberate failure and independent recovery; the failed run removes the previous zero-exit artifact. |
| Current inventory | The mechanical verifier passes for 129 packages, five tools, 19 dotfile targets, one setting, 67 preferences, two repositories, two login items, and one executable, including all hook owners. |

## Evidence lifecycle

Cold preconditions are asserted before stage 0. Initial cold installation and optional reruns use separate log names and preserved collections. Post-adoption SHA, clean status, and mise version are required. Cask scenarios were rerun in one uninterrupted guest lifecycle after discovering that a restart had cleared temporary evidence; only the complete recollected set is included here.

All current `.exit` files were checked against their expected success or intentional-refusal outcomes. Focused reports contain PASS except for the legacy isolated report's retained IN_PROGRESS status described above and the deliberately preserved failure-lifecycle artifacts. The retained status is not rewritten after the fact; the newer fixture writes an explicit terminal PASS.

## Accepted boundaries

This bundle does not change the scoped-v1 decisions: signed-in MAS installation/reruns remain manual and unverified, Amp Desktop's GUI remains nonfunctional, authenticated OMP/Claude behavior remains manually verified only, and full preference UI/delayed selective-reversal behavior remains manual.

## Sanitization and integrity

Terminal control sequences were removed except from synthetic status-line color evidence. Trailing blank lines were normalized. App bundles, symlinks, host screenshots, personal data, and credentials are excluded. High-confidence private-key/token scans passed. `SHA256SUMS` covers every file in this directory except itself.
