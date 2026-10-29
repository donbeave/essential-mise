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
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh full-dry-run
VM_NAME=essential-mise-test-stage0 scripts/vm/run-test-scenario.sh preferences
```

The scenarios deliberately share one VM when they have dependencies. Use separate clones for independent clean-state tests. The stage-0 launcher uses a full reviewed commit, not mutable `main`; update that commit deliberately. Run `ai` before the full dry run so the native Ruby prerequisite exists. Review every log before assigning a release result; a completed scenario command is not a blanket release approval. MAS/account, paid-license, personal-credential, and authenticated-provider tests remain blocked without synthetic or explicitly approved test accounts.

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
```

The launcher resolves the named Tart VM, requires its dedicated SSH key and verified host key, and executes only guest `/opt/homebrew/bin/cua-driver mcp`. It has no host-local fallback. The smoke test performs MCP initialization, lists tools and guest apps, and saves a guest-originated desktop image plus protocol evidence under `/tmp`.

Register the launcher using the agent's documented MCP interface, for example:

```bash
codex mcp add essential-mise-vm -- \
  env VM_NAME=essential-mise-test-stage0 \
  /absolute/path/to/scripts/vm/connect-mcp.sh
```

Discover tool arguments from the guest's installed MCP server. Use accessibility results to choose targets, verify visible state after actions, and save screenshots/logs through the guest evidence directory. Do not invent coordinates or treat a returned command as proof of a successful interaction.
