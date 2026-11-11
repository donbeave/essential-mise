# VM testing

Runtime and graphical verification happens only inside a disposable Tart macOS guest driven with guest-side Cua Driver. The harness lives in `testing/vm/`; its [README](../testing/vm/README.md) is the command reference. Never run the application on the host Mac.

## Model

- Create a fresh working VM from a stopped baseline with the clone helper; scripts refuse unregistered VM names.
- Transfer only reviewed build artifacts into the guest; never mount the host home or project.
- Guests use the synthetic `admin`/`admin` credential and synthetic accounts.
- Collect `/tmp/essential-mise-evidence` before stopping a guest.

## Drivers take a config

Every driver that runs config-specific checks takes:

- `--config <OWNER/REPO|file://...>`: the config repository the guest adopts.
- `--config-dir <path>`: a host checkout of that repository, used to find scenarios.
- `--scenario <file>`: a bash fragment (path, or a name under `<config-dir>/tests/vm/scenarios/`) that defines commands with the driver helpers.
- `--bootstrap <path>`: test a local `bootstrap.sh` instead of the pinned commit.

The generic suite uses `templates/config-starter` as its config. Config-specific scenarios and verifiers belong in the config repository under `tests/vm/`.

## Testing an unreleased console

`bootstrap.sh` pins a released console, so `--bootstrap` tests only launcher changes. To test an unreleased console, build it, transfer the binary with `testing/vm/transfer-app.sh`, and run `essential-mise-console setup --config <source>` directly in the guest.

## Typical run

```bash
VM_REVIEWED_COMMIT=$(git rev-parse HEAD) VM_NAME=essential-mise-test-stage0 \
  testing/vm/run-test-scenario.sh --config OWNER/REPO stage0
```

Review every log; a completed scenario is not a release approval.
