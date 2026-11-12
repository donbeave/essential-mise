# Releasing

How a console release (`essential-mise-console vX.Y.Z`) and the launcher pin to it are produced. Only maintainers with access to the `release` environment can do this.

## Overview

| Step | Result | Where |
| --- | --- | --- |
| 1. Prepare the release PR by hand | Version bump merged to `main` | PR titled `Release essential-mise-console vX.Y.Z` |
| 2. Wait for CI on the merge commit | Green `CI` run on that exact commit | Actions |
| 3. Dispatch the release | Signed, attested GitHub release and tag `vX.Y.Z` | `Rust binary release` workflow, `operation=release` |
| 4. Pin the launcher | `bootstrap.sh` points at the new release | PR titled `chore: pin launcher to console vX.Y.Z` |
| 5. Bump `min_console` in config repositories that need it | Older consoles are refused | Your config repositories |

## Why the release PR is prepared by hand

The generated `Rust binary release` workflow has a `prepare` operation. It runs `release-plz release-pr`, which expects a Cargo workspace at the repository root. This repository's Cargo project lives in `apps/essential-mise-console/`, so `prepare` fails here (observed: `failed to update packages`, and `release-pr requires a branch` on a detached checkout). Do not use `operation=prepare`. The `release` operation does not need it: it reads the version from `apps/essential-mise-console/Cargo.toml` on `main`.

## 1. Prepare the release PR

1. Create a branch from the current `main`:

   ```bash
   git switch -c release/vX.Y.Z origin/main
   ```

2. Set the new version in `apps/essential-mise-console/Cargo.toml` (`version = "X.Y.Z"`). The crate is pre-1.0: bump the minor version for a change to the config contract or the console behavior, the patch version for fixes.
3. Update the lock file. `cargo` rewrites the crate's own entry in `apps/essential-mise-console/Cargo.lock`:

   ```bash
   cargo metadata --manifest-path apps/essential-mise-console/Cargo.toml --format-version 1 > /dev/null
   git diff --stat   # exactly Cargo.toml and Cargo.lock, one line each
   ```

4. Commit with `chore(release): X.Y.Z` and open a PR titled `Release essential-mise-console vX.Y.Z`. In the body, list the user-visible changes since the last release. Only the two version lines change.
5. Merge it with a merge commit when CI is green, like every other PR in this repository.

## 2. Wait for green CI

The release operation checks that the `CI` workflow succeeded on the exact commit it releases (the head of `main`). Wait for the `CI` run on the merge commit of step 1 to finish green before you dispatch. A failed flaky job (for example the visual test `s3_failure_modal_matches_baseline`, or a hung `Plan` job) can be rerun once; a repeat failure needs a fix first.

## 3. Dispatch the release

```bash
gh workflow run binary-release.yml --repo donbeave/essential-mise --ref main -f operation=release
gh run list --workflow binary-release.yml --repo donbeave/essential-mise --limit 1
gh run watch <run-id> --repo donbeave/essential-mise
```

Rules while the run is in progress:

- Do not merge anything to `main` until the run finishes. The run releases one source commit and requires green CI on exactly that commit; a merge in the middle makes the released commit ambiguous.
- The jobs that use the `release` environment (tag, signer, publisher) wait for a maintainer to approve the deployment. Expect to approve twice. Approve in the run page (**Review deployments**).
- The `original_run_id` and `original_run_attempt` inputs are for recovery only: use them together, to resume a run that built and signed a release but failed to publish. Leave them empty otherwise.

The run builds `aarch64-apple-darwin` on a macOS runner (`scripts/emc-release-package.sh`, task `release:package`), signs and attests the archive, creates the tag `vX.Y.Z`, and publishes a release with these assets:

| Asset | Purpose |
| --- | --- |
| `essential-mise-console-aarch64-apple-darwin.tar.gz` | The binary. Its SHA-256 is the launcher pin. |
| `essential-mise-console-aarch64-apple-darwin.tar.gz.sha256` | SHA-256 of the archive. |
| `essential-mise-console-aarch64-apple-darwin.tar.gz.sbom.json` | CycloneDX SBOM. |
| `essential-mise-console-aarch64-apple-darwin.tar.gz.bundle`, `release.json.bundle` | Attestation bundles. |
| `release.json`, `SHA256SUMS` | Release manifest and checksum list. |

## 4. Pin the launcher

`bootstrap.sh` pins one console release with two variables. Until you change them, users keep getting the old console.

1. Download the archive in a clean directory, verify its attestation, and compute the checksum:

   ```bash
   dir="$(mktemp -d)"
   gh release download vX.Y.Z --repo donbeave/essential-mise \
     --pattern 'essential-mise-console-aarch64-apple-darwin.tar.gz*' --pattern SHA256SUMS --dir "$dir"
   gh attestation verify "$dir/essential-mise-console-aarch64-apple-darwin.tar.gz" --repo donbeave/essential-mise
   shasum -a 256 "$dir/essential-mise-console-aarch64-apple-darwin.tar.gz"
   cat "$dir/essential-mise-console-aarch64-apple-darwin.tar.gz.sha256"
   ```

   `gh attestation verify` prints nothing and exits 0 on success. The `shasum` output and the `.sha256` asset (and the matching line in `SHA256SUMS`) must be identical. Hash the `.tar.gz` file itself, not the extracted binary.

2. On a new branch, edit the two lines at the top of `bootstrap.sh`:

   ```bash
   EMC_VERSION='vX.Y.Z'
   EMC_SHA256='<64 hex characters>'
   ```

3. Open a PR titled `chore: pin launcher to console vX.Y.Z`. State in the body that the SHA-256 matches the published `.sha256` asset and that the attestation verified.
4. Because the launcher runs on Macs, test the change in a disposable VM before you merge: 

   ```bash
   VM_NAME=essential-mise-test-stage0 \
     testing/vm/run-test-scenario.sh --bootstrap bootstrap.sh --config OWNER/REPO stage0
   ```

   The VM must be a fresh registered clone; see [VM testing](vm-testing.md).
5. Merge when CI is green. The launcher on `main` is what the public one-liner downloads, so this merge is the moment users move to the new console.

## 5. Bump `min_console` in config repositories

A config repository that uses a feature of the new console should require it. In each such repository, set the header in `config.toml` to the released version and run lint with the new console:

```toml
#: min_console = "X.Y.Z"
```

Consoles older than that stop at setup stage 5 and tell the user to rerun the latest `bootstrap.sh`. See [Bumping `min_console`](config-repo.md#bumping-min_console).

## Generated workflows

Everything in `.github/workflows/` is generated by Velnor Actions from `.velnor/config.toml`. Never edit a workflow by hand, and do not add custom files to that directory.

To change CI or release behavior:

1. Edit `.velnor/config.toml` (and `mise.toml` only when the release producer task changes).
2. Regenerate with the Velnor generator: `velnor-actions generate`. The generator version is recorded in the first line of each workflow and in `.velnor/release-manifest.json`.
3. Commit the config change and the regenerated files together.

If the generated output is wrong, fix the generator upstream rather than patching the workflow; see `.github/AGENTS.md`.
