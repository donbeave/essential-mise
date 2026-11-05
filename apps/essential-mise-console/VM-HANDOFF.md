# VM handoff — essential-mise-console 2-module apply (T5.3)

Executor stops here. Do NOT run the steps below; they need an operator
(Tart control + guest credentials/prompts). Blocked on operator.

## What to run

Binary: first signed release of `essential-mise-console` (T1.4, pending),
or a debug build transferred into the guest. Config: the operator's
`essential-mise` checkout (this repo) as `MISE_CONFIG_DIR`.

Module selection (exact, deterministic):
- S1: select `terminal` + `shell` (Space on both), Enter.
- S2: keep all (pre-ticked), Enter. Expected keep-list includes
  `brew:starship`, `brew:bat`, `npm:pyright` (origin `config.*.toml`).
- S3: Enter starts the worker; watch the live log + progress. No cancel.
- S4: auto-refresh must show drift resolved for the two envs.
  `Done` exits 0.

## VM recipe

- Base: local Tart VM `baseline-27-2` (macOS 27.2, stopped). Clone, don't
  touch the baseline: `tart clone baseline-27-2 emc-test-1`.
- Boot headless: `tart run --no-graphics emc-test-1 &`, then `tart ip emc-test-1`.
- Guest account: `admin` / `admin` (synthetic; never personal credentials).
- Transfer the binary (scp to `admin@<ip>`) plus this repo (or a release
  tarball); never mount the host home directory.
- Drive the TUI with cua-driver 0.38.0 (`cua-driver serve/call` over SSH);
  fall back to plain ssh for log collection.
- Real installs happen ONLY here — never on the host (plan R1).

## Evidence to bring back

- Screenshots or Cua log of the S1→S4 walk (2-module selection above).
- `S4 Result` screen state (all-ok, exit code of the run).
- The full apply log from the S3 temp file + the rerun command, if any.
- `sudo`/TCC prompts encountered (if any) and how they were approved.
- Cleanup afterwards: `tart stop emc-test-1; tart delete emc-test-1`.
- Never exceed two running Tart VMs; never overwrite `baseline-27-2`.
