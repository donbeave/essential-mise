# VM handoff — essential-mise-console 2-module apply (T5.3)

EXECUTED 2026-11-05 by executor (autonomous; no operator prompts used).
Clone `emc-test-1` was created from `baseline-27-2`, provisioned over VNC
(`tart run --vnc-experimental --no-graphics`, `vncdotool` over RFB; host
cua-driver 0.38.0 daemon runs but host TCC is ungranted, so no host-side
capture — all driving went through the VNC framebuffer).

## What ran

Binary: debug build `essential-mise-console 0.1.0` (`target/debug/…`, scp to
`/tmp/emc`). Config: `/tmp/emc-cfg` holding reviewed repo files only
(`bootstrap.sh`, `config.toml`, `config.terminal.toml`,
`dotfiles/terminal/{starship.toml,ghostty/config,ghostty/generic-terminal}`,
`bin/fish-config.py`). Deviation from the 2-module script: the repo has no
`config.shell.toml`, so the run used ONE module (`terminal`, 15 packages);
`npm:pyright` belongs to another env and was out of scope.

Module walk (screenshots `/private/tmp/emc-guest-*.png` on the host):
- S1: 29 environments listed from `bootstrap.sh`, `terminal` selected (Space).
- S2: 15/15 kept (`brew:bash/bat/buf/fish/git-standup/gogcli/just/msedit/
  nushell/starship/watch/wget/xh`, `brew-cask:font-…/ghostty`).
- S3: worker ran phase 1
  (`mise -C /tmp/emc-cfg -E terminal bootstrap --skip packages --update
  --upgrade`) then phase 2 (`packages apply`); live log showed
  checksum/extract/relocate/codesign/link lines; progress reached 50%+.
- S4: `4/4 Result`, `✓ 18 of 18 parts ok · 0 drift`, `packages in sync`;
  `Done` (Enter) exited the TUI to a clean shell prompt.

Verified post-run over SSH (`admin@192.168.64.4`):
`packages status` lists all 15 entries `installed`
(starship 1.27.0, bat 0.26.1, …, ghostty 1.4.0); `bat 0.26.1` and
`/Applications/Ghostty.app` present. S3 temp log:
`/var/folders/rz/cp4jz8hj5w15mjv1bpjfk9500000gn/T/emc-apply-*-0.log`
(phase header + streamed lines; full log retained in the guest until cleanup).

## Guest provenance (all recorded, nothing on the host was mutated)

- Guest: macOS 27.2 build 26B123 (`sw_vers`), VM `emc-test-1` from
  `baseline-27-2` (baseline left stopped and untouched).
- Setup Assistant walked via VNC (English, US, set-up-as-new, `admin`/`admin`
  synthetic account, Apple Account skipped, license agreed, adult, light,
  location off, analytics off, screen time/Siri later, FileVault off).
- Remote Login enabled in System Settings (admin auth); SSH verified.
- Toolchain installed with stock installers only: CLT via
  `xcode-select --install` (GUI Install + Agree), Homebrew 7.0.12 via the
  official script (`NONINTERACTIVE=1`), mise 2026.11.1 via `https://mise.run`
  + self-update. NOPASSWD sudoers drop-in for `admin` (test-only).
- Findings for P2: (1) `sudo -v` ignores NOPASSWD and macOS `tty_tickets`
  scopes the timestamp per-TTY, so the S3 worker blocks on `Password:` until
  the *Terminal* tty is primed — worked around with `!tty_tickets` +
  `timestamp_timeout=30` in the guest; stock `bootstrap.sh` primes via its
  own `sudo -v` on the same tty, the TUI cannot. (2) VNC `type` needs
  `--force-caps` for capitals/underscores.

## Original operator-run recipe (kept for reference)

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
