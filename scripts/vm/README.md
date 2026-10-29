# Isolated macOS verification

Tart is the only VM manager. Cua Driver runs inside the guest and exposes MCP over SSH. Never launch the application under test on the host, grant host automation permissions, share host directories, or use host screenshots as verification evidence.

## Prerequisites

Use an Apple Silicon host running macOS 27.2, with at least 60 GB free. Install Tart through Homebrew:

```bash
brew install openai/tools/tart
tart --version
```

The scripts require macOS `expect`, Python 3, and a full Xcode installation is not required. They use 4 guest CPUs, 8192 MB RAM, a 100 GB disk, NAT networking, no host directory shares, and no clipboard sharing. NAT provides connectivity, not complete network isolation.

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

Approve Cua Driver's Accessibility, Screen Recording, System Events, and direct screen-capture prompts **inside the guest only**. The driver command can time out while waiting; approve the grants and rerun it. Do not edit TCC databases, disable SIP, or bypass Gatekeeper.

After consent succeeds:

```bash
scripts/vm/finalize-automation-baseline.sh
```

Finalization requires the exact guest OS, driver version/status, doctor, application listing, and a real guest desktop capture. It stops and preserves the verified baseline. A clone did not retain `authorized_keys` during initial setup; `ensure-guest-access.sh` restores only the dedicated public key using the synthetic owner password and verifies key login. Never weaken host-key checking to work around this.

Homebrew is the default driver source at `/opt/homebrew/bin/cua-driver`. For a genuinely fresh guest without Homebrew, download and inspect the official Cua installer and helper, then use the absolute `VM_CUA_DRIVER` override, for example `VM_CUA_DRIVER=/Users/admin/.local/bin/cua-driver`. Verify the release SHA-256 and Developer ID/notarization. The override changes only the guest executable path and never permits a host-local driver or symlink into another package owner.

Tart allows at most two running VMs. Stop one disposable VM before starting another; baselines remain stopped during cloning and ordinary tests.

## Disposable test VMs

Create and start a workflow-owned clone:

```bash
scripts/vm/clone-disposable-vm.sh essential-mise-test-stage0
VM_NAME=essential-mise-test-stage0 \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/start-test-vm.sh
VM_NAME=essential-mise-test-stage0 scripts/vm/check-vm-ready.sh
```

The clone helper records ownership under `~/.config/essential-mise/vm/disposable-vms`. Scripts refuse unrelated VM names and unregistered disposable VMs. The source baseline must remain stopped during cloning.

Transfer only a reviewed artifact, never a home or project directory:

```bash
VM_NAME=essential-mise-test-stage0 \
  scripts/vm/transfer-app.sh /tmp/essential-mise-release.tar.gz
VM_NAME=essential-mise-test-stage0 \
  scripts/vm/collect-evidence.sh /tmp/essential-mise-evidence/stage0
```

Guest artifacts are limited to `/tmp/essential-mise-artifact`; guest evidence is collected from `/tmp/essential-mise-evidence`.

Run application scenarios only in a registered disposable VM:

```bash
VM_REVIEWED_COMMIT=$(git rev-parse HEAD) VM_NAME=essential-mise-test-stage0 \
  scripts/vm/run-test-scenario.sh stage0
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh standard
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh ai
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh ai-desktop
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh amp-conflict
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh claude-conflict
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh full-dry-run
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh preferences
```

The scenarios deliberately share one VM when they have dependencies. Use separate clones for independent clean-state tests. The stage-0 launcher uses a full reviewed commit, not mutable `main`; update that commit deliberately. Run `ai` before the full dry run to exercise the real-Homebrew hooks and eliminate package conflicts. Review every log before assigning a release result; a completed scenario command is not a blanket release approval. MAS/account, paid-license, personal-credential, and authenticated-provider tests remain blocked without synthetic or explicitly approved test accounts.

Replay the isolated first-run, optional-selection rerun, wrong-origin, and XDG fixtures:

```bash
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/isolated-validation.sh
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "REVIEWED_COMMIT=$(git rev-parse HEAD) /bin/bash /tmp/essential-mise-artifact/isolated-validation.sh"
```

The fixture clones the reviewed public commit inside the guest, substitutes its local repository path only in a temporary copy of `bootstrap.sh`, and records every invocation, exit code, and log under `/tmp/essential-mise-evidence/isolated-fixtures`. Temporary HOME/XDG directories are created explicitly. No real global configuration is used.

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
