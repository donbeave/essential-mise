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
  testing/vm/create-fresh-baseline.sh
```

This downloads Apple's latest IPSW, requires the installed guest to report exactly macOS 27.2 (26B123), verifies arm64, SIP and Gatekeeper, then stops `essential-mise-fresh-baseline`. No personal Apple Account or credentials are used. Tart's first-boot provisioning option receives the password without printing it; inspect the Tart process security implications before use. The script refuses to overwrite an existing baseline.

## Automation baseline

```bash
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  testing/vm/create-automation-baseline.sh
```

This clones the fresh baseline, restores SSH access if necessary, installs Homebrew only when absent, and installs the current `cuadriver` cask. The reviewed Homebrew installer is pinned by SHA-256; a changed installer fails closed for review. `prepare-automation-baseline.sh` is the idempotent resume path and never clones over an existing VM.

In the Tart window for `essential-mise-automation-baseline`, run:

```bash
VM_NAME=essential-mise-automation-baseline testing/vm/grant-cua-permissions.sh
```

Approve Cua Driver's Accessibility, Screen Recording, System Events, and direct screen-capture prompts **inside the guest only**. The helper restarts the daemon through guest LaunchServices (`open -n -g -a CuaDriver --args serve`), so TCC attributes the grants to `com.trycua.driver` rather than the SSH session. The driver command can time out while waiting; approve the grants and rerun it. Do not edit TCC databases, disable SIP, or bypass Gatekeeper. A local Tart VNC connection can automate the initial privacy grants before Cua is ready; keep its generated endpoint credential transient and out of evidence, and use Cua for application verification afterward.

After consent succeeds:

```bash
testing/vm/finalize-automation-baseline.sh
```

Finalization requires the exact guest OS, driver version/status, doctor, application listing, and a real guest desktop capture. It stops and preserves the verified baseline. A clone did not retain `authorized_keys` during initial setup; `ensure-guest-access.sh` restores only the dedicated public key using the synthetic owner password and verifies key login. Never weaken host-key checking to work around this.

Homebrew is the default driver source at `/opt/homebrew/bin/cua-driver`. For a genuinely fresh guest without Homebrew, download and inspect the official Cua installer and helper, then use the absolute `VM_CUA_DRIVER` override, for example `VM_CUA_DRIVER=/Users/admin/.local/bin/cua-driver`. Verify the release SHA-256 and Developer ID/notarization. The override changes only the guest executable path and never permits a host-local driver or symlink into another package owner.

Tart allows at most two running VMs. Stop one disposable VM before starting another; baselines remain stopped during cloning and ordinary tests.

## Disposable test VMs

For the cold `stage0` workflow, explicitly clone the fresh baseline, not the default automation baseline, and start the workflow-owned clone:

```bash
VM_SOURCE_BASELINE=essential-mise-fresh-baseline \
  testing/vm/clone-disposable-vm.sh essential-mise-test-stage0
VM_NAME=essential-mise-test-stage0 \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  testing/vm/start-test-vm.sh
```

Before readiness checks, use `owner-auth-ssh.sh` to download, inspect, and install the official Cua Driver release entirely inside this fresh guest at `/Users/admin/.local/bin/cua-driver`, without installing Homebrew. Verify the release SHA-256 and Developer ID/notarization as required above, then approve the guest-only Cua permissions. Pass the absolute guest driver override to both helpers:

```bash
VM_NAME=essential-mise-test-stage0 \
VM_CUA_DRIVER=/Users/admin/.local/bin/cua-driver \
  testing/vm/grant-cua-permissions.sh
VM_NAME=essential-mise-test-stage0 \
VM_CUA_DRIVER=/Users/admin/.local/bin/cua-driver \
  testing/vm/check-vm-ready.sh
```

For prepared scenarios that do not assert absent Homebrew, keep the default automation baseline and its preinstalled driver:

```bash
testing/vm/clone-disposable-vm.sh essential-mise-test-evidence
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  testing/vm/start-test-vm.sh
VM_NAME=essential-mise-test-evidence testing/vm/check-vm-ready.sh
```

The clone helper records ownership under `~/.config/essential-mise/vm/disposable-vms`. Scripts refuse unrelated VM names and unregistered disposable VMs. The source baseline must remain stopped during cloning.

Transfer only a reviewed artifact, never a home or project directory:

```bash
VM_NAME=essential-mise-test-stage0 \
  testing/vm/transfer-app.sh /tmp/essential-mise-release.tar.gz
VM_NAME=essential-mise-test-stage0 \
  testing/vm/collect-evidence.sh /tmp/essential-mise-evidence/stage0
```

Guest artifacts are limited to `/tmp/essential-mise-artifact`; guest evidence is collected from `/tmp/essential-mise-evidence`. Collect it before stopping or rebooting a guest because temporary evidence may be removed during the guest lifecycle. Keep generated logs, screenshots, and fixture output outside the repository rather than committing an evidence archive. Collection replaces only destinations carrying its ownership marker, rather than merging stale logs, and refuses unrelated nonempty directories. Scenario logs include exact command exit codes, the reviewed/config revisions, fixture checksum, mise/Tart versions, and guest OS/build.

Run application scenarios only in a registered disposable VM. The driver owns the VM-name guard, evidence directory, fixture checksum and commit pin; a scenario is a bash fragment that only defines commands using the driver helpers (`run_logged`, `run_logged_interactive`, `vm_ssh`, `record_config_state`, `$config_dir`, `$evidence_dir`, `$stage0_command`). It is sourced on the host by the driver and never executed in the guest as a whole.

Common options for the drivers that run config-specific checks:

- `--config <OWNER/REPO|file://...>` the config repo the guest adopts (or `ESSENTIAL_MISE_CONFIG`).
- `--config-dir <host path>` a checkout of that config repo, used to locate `tests/vm/scenarios/` (or `ESSENTIAL_MISE_CONFIG_DIR`).
- `--scenario <file>` a scenario fragment, either a path or a file name inside `<config-dir>/tests/vm/scenarios/`.
- `--bootstrap <local path>` use a local `bootstrap.sh` instead of downloading it at `VM_REVIEWED_COMMIT`. An unreleased bootstrap still pins a released console (`EMC_VERSION`), so this only tests `bootstrap.sh` changes; to test an unreleased console, run the built binary directly (see `docs/vm-testing.md`).

Run `stage0` only on a fresh clone with no Homebrew or global config. It asserts those cold preconditions, preserves the successful initial transaction, and asserts that stage 0 reaches the final stage, that a rerun converges and that a wrong origin is refused:

```bash
VM_REVIEWED_COMMIT=$(git rev-parse HEAD) VM_NAME=essential-mise-test-stage0 \
VM_CUA_DRIVER=/Users/admin/.local/bin/cua-driver \
  testing/vm/run-test-scenario.sh --config OWNER/REPO stage0
```

Continue on that configured guest with a scenario from the config repo, for example:

```bash
VM_NAME=essential-mise-test-stage0 \
  testing/vm/run-test-scenario.sh --config OWNER/REPO --config-dir "$C" \
  --scenario "$C/tests/vm/scenarios/<name>.sh"
```

The stage-0 launcher uses a full reviewed commit, not mutable `main`; update that commit deliberately. Review every log before assigning a release result; a completed scenario command is not a blanket release approval. MAS/account, paid-license, personal-credential, and authenticated-provider tests remain blocked without synthetic or explicitly approved test accounts.

Generic verifiers that are parametrized by a config repo stay in this directory. Each standalone fixture clears its previous evidence and records IN_PROGRESS or FAIL instead of retaining an earlier PASS:

```bash
VM_NAME=essential-mise-test-evidence testing/vm/transfer-app.sh testing/vm/verify-configuration-safety.py
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  testing/vm/owner-auth-ssh.sh \
  "/usr/bin/python3 /tmp/essential-mise-artifact/verify-configuration-safety.py --config-dir \$HOME/.config/mise"
VM_NAME=essential-mise-test-evidence testing/vm/transfer-app.sh testing/vm/verify-project-boundaries.py
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  testing/vm/owner-auth-ssh.sh \
  "/usr/bin/python3 /tmp/essential-mise-artifact/verify-project-boundaries.py --bootstrap /tmp/essential-mise-artifact/bootstrap.sh"
```

`verify-configuration-safety.py` accepts `--env-git`, `--git-ignore-source`, `--expect KEY=VALUE`, `--env-editor`, `--editor-settings-path` and `--editor-absent`; sections without their option report `SKIP`. Output is under `/tmp/essential-mise-evidence/configuration-safety`. `verify-project-boundaries.py` accepts `--env-a/--env-b/--env-c` (defaults are modules of the starter config), `--base-modules` and the optional `--env-workspaces`; its output covers selector precedence, project resource and environment isolation, optional workspace activation, and nested first adoption and rerun that must exclude a trusted parent project's repository and hook while an unbounded control sees both.

Replay the isolated first-run, optional-selection rerun, wrong-origin and XDG fixtures against a config repo:

```bash
VM_NAME=essential-mise-test-evidence testing/vm/transfer-app.sh testing/vm/isolated-validation.sh
VM_AUTO_CONFIRM=1 VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  testing/vm/owner-auth-ssh.sh \
  "REVIEWED_COMMIT=<config sha> /bin/bash /tmp/essential-mise-artifact/isolated-validation.sh --config OWNER/REPO"
```

The fixture clones the reviewed config commit inside the guest, runs the engine `bootstrap.sh` against that local clone, and records every invocation, exit code, and log under `/tmp/essential-mise-evidence/isolated-fixtures`. Temporary HOME/XDG directories are created explicitly under `/private/tmp`; no real global configuration is used. It covers non-Git configuration preservation and subsequent adoption, existing-Homebrew empty `shellenv`, path aliases, symlinked ancestors, bare repositories, repository subdirectories, linked worktrees, wrong origins, exact adopted revisions, and an update/apply lifecycle on a custom XDG directory. A config repo can add module-specific checks with `--scenario`: the sourced fragment defines an `isolated_scenario` function that runs after the generic checks and may use `run_and_record` and the `xdg_*` variables. Evidence is cleared on every run and failed runs overwrite the summary with FAIL.

Delete only a registered disposable VM after its test is complete:

```bash
testing/vm/reset-disposable-vm.sh essential-mise-test-stage0
```

Baselines and the working VM are never accepted by the reset helper. Do not delete baselines as part of ordinary tests.

## Guest MCP over SSH

For an already running, verified VM:

```bash
VM_NAME=essential-mise-test-stage0 testing/vm/connect-mcp.sh
VM_NAME=essential-mise-test-stage0 testing/vm/smoke-mcp.sh
VM_NAME=essential-mise-test-stage0 testing/vm/smoke-ui.py
```

The launcher resolves the named Tart VM, requires its dedicated SSH key and verified host key, and executes only guest `/opt/homebrew/bin/cua-driver mcp`. It has no host-local fallback. The MCP smoke test performs initialization, lists tools and guest apps, and saves a guest-originated desktop image plus protocol evidence under `/tmp`. `smoke-ui.py` uses session-scoped accessibility tokens to calculate 6×7 in guest Calculator, asserts 42 in the window tree, and saves initial/final screenshots in the guest evidence directory.

Register the launcher through your agent's MCP interface, with `VM_NAME=essential-mise-test-stage0` in its environment and the absolute path of `testing/vm/connect-mcp.sh` as its command.

Discover tool arguments from the guest's installed MCP server. Use accessibility results to choose targets, verify visible state after actions, and save screenshots/logs through the guest evidence directory. Do not invent coordinates or treat a returned command as proof of a successful interaction.

Cua's background `launch_app` can leave some macOS apps, such as terminal emulators, without an initial window. In that case, use `/usr/bin/open -a <AppName>` inside the guest (never on the host), then inspect the new window through Cua. Use `caffeinate -u -t 1` inside the guest to wake an idle display before UI verification; this does not bypass lock-screen security or TCC. Handle routine prompts autonomously with `admin`/`admin` and verify the resulting state rather than trusting click responses.
