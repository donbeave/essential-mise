# Isolated macOS verification

Tart is the only VM manager. Cua Driver runs inside the guest and exposes MCP over SSH. Never launch the application under test on the host, grant host automation permissions, share host directories, or use host screenshots as verification evidence.

## Prerequisites

Use an Apple Silicon host running macOS 27.2, with at least 60 GB free. Install Tart through Homebrew:

```bash
brew install openai/tools/tart
tart --version
```

The scripts require macOS `expect` and Python 3; a full Xcode installation is not required. Most fixtures use macOS's `/usr/bin/python3`. The inventory checker additionally requires Python 3.11 or newer for `tomllib`; macOS 27.2's system Python is 3.9, so install the contributor-only dependency with `brew install python3` and invoke `/opt/homebrew/bin/python3`. The isolated lifecycle also invokes `/opt/homebrew/bin/fish`, which can be installed with `brew install fish`. Test guests use 4 CPUs, 8192 MB RAM, a 100 GB disk, NAT networking, no host directory shares, and no clipboard sharing. NAT provides connectivity, not complete network isolation.

Create a dedicated SSH identity and the shared synthetic guest-owner credential file. Every test guest uses username `admin` and password `admin`; never use personal credentials or generate unique guest passwords. The credential file is not part of the repository, must have mode 600, and is never printed by the scripts:

```bash
mkdir -p ~/.config/essential-mise/vm
chmod 700 ~/.config/essential-mise/vm
ssh-keygen -t ed25519 -N '' -C essential-mise-isolated-guest \
  -f ~/.config/essential-mise/vm/id_ed25519
printf 'admin\n' > /tmp/essential-mise-vm-owner-password
chmod 600 /tmp/essential-mise-vm-owner-password
```

Preserve existing identities or VMs instead of overwriting them. Use only `admin` / `admin` for guest authentication in this disposable test environment.

## Fresh stock-security baseline

```bash
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/create-fresh-baseline.sh
```

This downloads Apple's latest IPSW, requires the installed guest to report exactly macOS 27.2 (26B123), verifies arm64, SIP and Gatekeeper, then stops `essential-mise-fresh-baseline`. No personal Apple Account or credentials are used. Tart's first-boot provisioning option receives the password without printing it; inspect the Tart process security implications before use. The script refuses to overwrite an existing baseline.

## Automation baseline

```bash
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/create-automation-baseline.sh
```

This clones the fresh baseline, restores SSH access if necessary, installs Homebrew only when absent, and installs the current `cuadriver` cask. The reviewed Homebrew installer is pinned by SHA-256; a changed installer fails closed for review. `prepare-automation-baseline.sh` is the idempotent resume path and never clones over an existing VM.

In the Tart window for `essential-mise-automation-baseline`, run:

```bash
VM_NAME=essential-mise-automation-baseline scripts/vm/grant-cua-permissions.sh
```

Approve Cua Driver's Accessibility, Screen Recording, System Events, and direct screen-capture prompts **inside the guest only**. The helper restarts the daemon through guest LaunchServices (`open -n -g -a CuaDriver --args serve`), so TCC attributes the grants to `com.trycua.driver` rather than the SSH session. The driver command can time out while waiting; approve the grants and rerun it. Do not edit TCC databases, disable SIP, or bypass Gatekeeper. A local Tart VNC connection can automate the initial privacy grants before Cua is ready; keep its generated endpoint credential transient and out of evidence, and use Cua for application verification afterward.

After consent succeeds:

```bash
scripts/vm/finalize-automation-baseline.sh
```

Finalization requires the exact guest OS, driver version/status, doctor, application listing, and a real guest desktop capture. It stops and preserves the verified baseline. A clone did not retain `authorized_keys` during initial setup; `ensure-guest-access.sh` restores only the dedicated public key using the synthetic owner password and verifies key login. Never weaken host-key checking to work around this.

Homebrew is the default driver source at `/opt/homebrew/bin/cua-driver`. For a genuinely fresh guest without Homebrew, download and inspect the official Cua installer and helper, then use the absolute `VM_CUA_DRIVER` override, for example `VM_CUA_DRIVER=/Users/admin/.local/bin/cua-driver`. Verify the release SHA-256 and Developer ID/notarization. The override changes only the guest executable path and never permits a host-local driver or symlink into another package owner.

Tart allows at most two running VMs. Stop one disposable VM before starting another; baselines remain stopped during cloning and ordinary tests.

## Disposable test VMs

For the cold `stage0` workflow, explicitly clone the fresh baseline, not the default automation baseline, and start the workflow-owned clone:

```bash
VM_SOURCE_BASELINE=essential-mise-fresh-baseline \
  scripts/vm/clone-disposable-vm.sh essential-mise-test-stage0
VM_NAME=essential-mise-test-stage0 \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/start-test-vm.sh
```

Before readiness checks, use `owner-auth-ssh.sh` to download, inspect, and install the official Cua Driver release entirely inside this fresh guest at `/Users/admin/.local/bin/cua-driver`, without installing Homebrew. Verify the release SHA-256 and Developer ID/notarization as required above, then approve the guest-only Cua permissions. Pass the absolute guest driver override to both helpers:

```bash
VM_NAME=essential-mise-test-stage0 \
VM_CUA_DRIVER=/Users/admin/.local/bin/cua-driver \
  scripts/vm/grant-cua-permissions.sh
VM_NAME=essential-mise-test-stage0 \
VM_CUA_DRIVER=/Users/admin/.local/bin/cua-driver \
  scripts/vm/check-vm-ready.sh
```

For prepared scenarios that do not assert absent Homebrew, keep the default automation baseline and its preinstalled driver:

```bash
scripts/vm/clone-disposable-vm.sh essential-mise-test-evidence
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/start-test-vm.sh
VM_NAME=essential-mise-test-evidence scripts/vm/check-vm-ready.sh
```

The clone helper records ownership under `~/.config/essential-mise/vm/disposable-vms`. Scripts refuse unrelated VM names and unregistered disposable VMs. The source baseline must remain stopped during cloning.

Transfer only a reviewed artifact, never a home or project directory:

```bash
VM_NAME=essential-mise-test-stage0 \
  scripts/vm/transfer-app.sh /tmp/essential-mise-release.tar.gz
VM_NAME=essential-mise-test-stage0 \
  scripts/vm/collect-evidence.sh /tmp/essential-mise-evidence/stage0
```

Guest artifacts are limited to `/tmp/essential-mise-artifact`; guest evidence is collected from `/tmp/essential-mise-evidence`. Collect it before stopping or rebooting a guest because temporary evidence may be removed during the guest lifecycle. Keep generated logs, screenshots, and fixture output outside the repository rather than committing an evidence archive. Collection replaces only destinations carrying its ownership marker, rather than merging stale logs, and refuses unrelated nonempty directories. Scenario logs include exact command exit codes, the reviewed/config revisions, fixture checksum, mise/Tart versions, and guest OS/build.

Run application scenarios only in a registered disposable VM:

Run `stage0` only on a fresh clone with no Homebrew or global config; it asserts those cold preconditions, preserves the successful initial transaction, and requires the adopted configuration SHA to equal `VM_REVIEWED_COMMIT`:

```bash
VM_REVIEWED_COMMIT=$(git rev-parse HEAD) VM_NAME=essential-mise-test-stage0 \
VM_CUA_DRIVER=/Users/admin/.local/bin/cua-driver \
  scripts/vm/run-test-scenario.sh stage0
```

Continue on that configured guest in this dependency order:

```bash
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh standard
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh ai
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh ai-desktop
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh claude-conflict
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh unapply
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh prune-review
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh js
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh java
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh rust
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh login-items
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh omp-config
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh mas-signed-out
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh optional-modules
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh surge-conflict
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh full-dry-run
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh preferences
```

Run Amp conflict on a **separate** fresh configured guest that has not run `ai-desktop`; the scenario asserts that neither the app nor its Homebrew owner exists before creating the unmanaged marker:

```bash
VM_REVIEWED_COMMIT=$(git rev-parse HEAD) VM_NAME=essential-mise-test-amp-owner \
  scripts/vm/run-test-scenario.sh stage0
VM_NAME=essential-mise-test-amp-owner scripts/vm/run-test-scenario.sh amp-conflict
```

The scenarios deliberately share one VM only when their dependencies match. The stage-0 launcher uses a full reviewed commit, not mutable `main`; update that commit deliberately. Run `ai` before the full dry run to exercise the real-Homebrew hooks and eliminate package conflicts. `surge-conflict` must follow network/optional installation. The `optional-modules` scenario models a first, nonactivated shell, so MAS entries are skipped while non-MAS packages install; its login-shell status exposes the remaining App Store gap rather than reporting complete success. Review every log before assigning a release result; a completed scenario command is not a blanket release approval. MAS/account, paid-license, personal-credential, and authenticated-provider tests remain blocked without synthetic or explicitly approved test accounts.

After applying the current `ai-cli` module, replay the synthetic Claude status-line and wrapper checks separately. The wrapper fixture verifies JSON encoding, shell quoting with a special-character XDG path, argument forwarding, and execution of the emitted command:

```bash
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/verify-claude-statusline.py
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/usr/bin/python3 /tmp/essential-mise-artifact/verify-claude-statusline.py"
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/verify-claude-wrapper.py
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/usr/bin/python3 /tmp/essential-mise-artifact/verify-claude-wrapper.py"
```

Replay Git preservation, custom-XDG ignore, and Zed JSONC checks, then verify actual GitHub CLI, Ghostty, OpenCode, and Fish behavior under custom XDG separately. Each standalone fixture clears its previous evidence and records IN_PROGRESS or FAIL instead of retaining an earlier PASS:

```bash
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/verify-configuration-safety.py
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/usr/bin/python3 /tmp/essential-mise-artifact/verify-configuration-safety.py"
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/verify-xdg-applications.py
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/usr/bin/python3 /tmp/essential-mise-artifact/verify-xdg-applications.py"
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/verify-hook-lifecycle.sh
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/bin/bash /tmp/essential-mise-artifact/verify-hook-lifecycle.sh"
VM_NAME=essential-mise-test-evidence \
  scripts/vm/owner-auth-ssh.sh \
  "/opt/homebrew/bin/python3 \$HOME/.config/mise/scripts/vm/verify-current-inventory.py"
VM_NAME=essential-mise-test-evidence \
  scripts/vm/owner-auth-ssh.sh \
  "/opt/homebrew/bin/python3 \$HOME/.config/mise/scripts/vm/verify-current-inventory.py --self-test"
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/verify-native-cask-coexistence.sh
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/bin/bash /tmp/essential-mise-artifact/verify-native-cask-coexistence.sh"
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/verify-project-boundaries.py
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/usr/bin/python3 /tmp/essential-mise-artifact/verify-project-boundaries.py"
```

The hook lifecycle fixture records structured outdated/formula metadata and proves the linked Amp CLI is behind before asserting that ordinary apply leaves its formula and link unchanged. If that cannot be established, it is BLOCKED, not PASS. Configuration-safety output is under `/tmp/essential-mise-evidence/configuration-safety`, and hook output is under `/tmp/essential-mise-evidence/hook-lifecycle`.

XDG application output is under `/tmp/essential-mise-evidence/xdg-applications`. It covers every GitHub CLI scalar, alias, and schema readback; `GH_CONFIG_DIR`; Ghostty effective configuration and theme validation; default/custom-XDG OpenCode instruction consumption, `OPENCODE_CONFIG_DIR` precedence and user-owned override preservation, and native unapply. OpenCode sends uniquely titled probes to a synthetic loopback-only provider. Exact user-message matching identifies every main request, whose bodies must contain the shared policy before removal and omit it afterward, with no external model or account. Also covers repeat convergence and unrelated-content preservation. Fish must resolve a preinstalled pinned Bun that was absent from its initial PATH. Neutralizing the activation body while preserving its markers and mise executable must remove that resolution. Amp completion has an equivalent external-completion exclusion and negative control.

The inventory checker validates source invariants without reading documentation; `--self-test` proves representative temporary mutations fail, including an installation outside its presence guard. The native-cask fixture checks Homebrew-owned Ghostty through ordinary native apply/reapply. Project-boundary output covers selector precedence, project resource/environment isolation, synthetic workspace activation, and all emitted stage-0 next-step commands run from a trusted project through a linked configuration worktree whose path contains spaces. Nested first adoption and foundation rerun must exclude a trusted parent project's repository and hook; an unbounded control must see both.

Replay Amp startup diagnostics in a separate registered guest, then collect evidence before stopping it:

```bash
VM_NAME=essential-mise-test-amp scripts/vm/grant-cua-permissions.sh
VM_NAME=essential-mise-test-amp scripts/vm/transfer-app.sh scripts/vm/diagnose-amp-startup.sh
VM_NAME=essential-mise-test-amp \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/bin/bash /tmp/essential-mise-artifact/diagnose-amp-startup.sh"
VM_NAME=essential-mise-test-amp \
  scripts/vm/collect-evidence.sh /tmp/essential-mise-amp-evidence
```

Replay the isolated first-run, optional-selection rerun, wrong-origin, and XDG fixtures:

```bash
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/isolated-validation.sh
VM_AUTO_CONFIRM=1 VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "REVIEWED_COMMIT=$(git rev-parse HEAD) /bin/bash /tmp/essential-mise-artifact/isolated-validation.sh"
```

The fixture clones the reviewed public commit inside the guest, substitutes its local repository path only in a temporary copy of `bootstrap.sh`, and records every invocation, exit code, and log under `/tmp/essential-mise-evidence/isolated-fixtures`. Temporary HOME/XDG directories are created explicitly under `/private/tmp`; no real global configuration is used. It covers non-Git configuration removal and adoption, existing-Homebrew empty `shellenv`, path aliases, symlinked ancestors, bare repositories, repository subdirectories, linked worktrees, wrong origins, exact adopted revisions, and a custom-XDG update/apply/unapply lifecycle. Its Fish check installs the pinned JS tools, starts with Bun absent from PATH, and requires interactive activation to resolve the exact mise-managed Bun. Evidence is cleared on every run and failed runs overwrite the summary with FAIL.

Replay the documented preference capture/apply/restore workflow:

```bash
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/verify-preference-roundtrip.py
VM_AUTO_CONFIRM=1 VM_NAME=essential-mise-test-evidence \
  VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "/usr/bin/python3 /tmp/essential-mise-artifact/verify-preference-roundtrip.py"
```

The fixture extracts the exact shell functions from `docs/manual-actions.md`. Stubbed checks cover existing and absent domains plus read/export/import and snapshot-directory creation failures. The real guest check captures all affected domains, applies `system-preferences`, checks stored status, restores immediately, compares every present domain with its snapshot, confirms original absence, and verifies unrelated nested and top-level values. Restoration is attempted even when an earlier assertion fails. Output is under `/tmp/essential-mise-evidence/preference-roundtrip` and is collected externally.

Delete only a registered disposable VM after its test is complete:

```bash
scripts/vm/reset-disposable-vm.sh essential-mise-test-stage0
```

Baselines and the working VM are never accepted by the reset helper. Do not delete baselines as part of ordinary tests.

## Guest MCP over SSH

For an already running, verified VM:

```bash
VM_NAME=essential-mise-test-stage0 scripts/vm/connect-mcp.sh
VM_NAME=essential-mise-test-stage0 scripts/vm/smoke-mcp.sh
VM_NAME=essential-mise-test-stage0 scripts/vm/smoke-ui.py
```

The launcher resolves the named Tart VM, requires its dedicated SSH key and verified host key, and executes only guest `/opt/homebrew/bin/cua-driver mcp`. It has no host-local fallback. The MCP smoke test performs initialization, lists tools and guest apps, and saves a guest-originated desktop image plus protocol evidence under `/tmp`. `smoke-ui.py` uses session-scoped accessibility tokens to calculate 6×7 in guest Calculator, asserts 42 in the window tree, and saves initial/final screenshots in the guest evidence directory.

Register the launcher using the agent's documented MCP interface, for example:

```bash
codex mcp add essential-mise-vm -- \
  env VM_NAME=essential-mise-test-stage0 \
  /absolute/path/to/scripts/vm/connect-mcp.sh
```

Discover tool arguments from the guest's installed MCP server. Use accessibility results to choose targets, verify visible state after actions, and save screenshots/logs through the guest evidence directory. Do not invent coordinates or treat a returned command as proof of a successful interaction.

Cua's background `launch_app` can leave some macOS apps, including Ghostty and Amp, without an initial window. In that case, use `/usr/bin/open -a <AppName>` inside the guest (never on the host), then inspect the new window through Cua. Use `caffeinate -u -t 1` inside the guest to wake an idle display before UI verification; this does not bypass lock-screen security or TCC. Handle routine prompts autonomously with `admin`/`admin` and verify the resulting state rather than trusting click responses.
