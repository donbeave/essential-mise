# Replayable isolated stage-0 and XDG fixtures

These fixtures make the earlier isolated first-run, rerun, wrong-origin, and alternate-XDG claims independently reproducible. They run inside a disposable macOS 27.2 guest and do not use a real global configuration.

## Invocation

Use a registered disposable VM and a full reviewed commit SHA:

```bash
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/isolated-validation.sh
VM_AUTO_CONFIRM=1 VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "REVIEWED_COMMIT=$(git rev-parse HEAD) /bin/bash /tmp/essential-mise-artifact/isolated-validation.sh"
```

`transfer-app.sh` copies only the reviewed script into the guest. `owner-auth-ssh.sh` supplies the test-only `admin`/`admin` credential through normal SSH/sudo prompts when needed. The script itself is `scripts/vm/isolated-validation.sh` in this repository.

## Fixture setup and local adoption substitution

The script:

1. Removes only `/private/tmp/essential-mise-isolated-validation` and `/tmp/essential-mise-evidence/isolated-fixtures`, initializes IN_PROGRESS, and writes FAIL on any unsuccessful exit. Earlier PASS artifacts cannot survive a failed rerun.
2. Clones the public repository into `/private/tmp/essential-mise-isolated-validation/repository`, checks out `REVIEWED_COMMIT`, and changes only the `REPO` assignment in a temporary launcher to substitute local-path adoption.
3. Uses explicit temporary HOME/XDG locations for every invocation. After local adoption, it asserts the exact adopted revision and sets public origin metadata for rerun checks without altering a real global checkout.
4. Verifies successful stage 0 when Homebrew's initialized PATH makes `shellenv` return an empty no-op, optional-selection rerun safety, wrong-origin refusal, HOME/root alias refusal, symlinked-ancestor refusal without adoption, bare-repository and repository-subdirectory refusal, and a valid linked worktree.
5. Exercises a real custom-XDG update by resetting the synthetic checkout to the reviewed commit's parent, capturing a nonempty diff, and fast-forwarding to the reviewed commit. It then previews and applies terminal dotfiles, verifies the copied file, checks a fresh login shell from an unrelated directory, and performs actual unapply/removal—not merely an empty preview.
6. Records each command's output and exact exit code in `/tmp/essential-mise-evidence/isolated-fixtures`.

## Evidence boundary

`isolated-fixtures/report.txt` identifies the reviewed SHA and each command's mise version. The current fixture's successful initial adoption, actual update, apply, and unapply logs must all be present before claiming the complete lifecycle. Historical evidence that reported only `Already up to date` or `nothing to remove` does not close those requirements.

Collected guest logs and `report.txt` are preserved under `docs/validation-evidence/<reviewed-commit>/isolated-fixtures` with SHA-256 checksums. Evidence is collected before stopping or rebooting the guest because its `/tmp` contents may be removed during the guest lifecycle.
