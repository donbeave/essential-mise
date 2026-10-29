# Replayable isolated stage-0 and XDG fixtures

These fixtures make the earlier isolated first-run, rerun, wrong-origin, and alternate-XDG claims independently reproducible. They run inside a disposable macOS 27.2 guest and do not use a real global configuration.

## Invocation

Use a registered disposable VM and a full reviewed commit SHA:

```bash
VM_NAME=essential-mise-test-evidence scripts/vm/transfer-app.sh scripts/vm/isolated-validation.sh
VM_NAME=essential-mise-test-evidence \
VM_OWNER_PASSWORD_FILE=/tmp/essential-mise-vm-owner-password \
  scripts/vm/owner-auth-ssh.sh \
  "REVIEWED_COMMIT=$(git rev-parse HEAD) /bin/bash /tmp/essential-mise-artifact/isolated-validation.sh"
```

`transfer-app.sh` copies only the reviewed script into the guest. `owner-auth-ssh.sh` supplies the test-only `admin`/`admin` credential through normal SSH/sudo prompts when needed. The script itself is `scripts/vm/isolated-validation.sh` in this repository.

## Fixture setup and local adoption substitution

The script:

1. Removes only `/tmp/essential-mise-isolated-validation` and creates explicit temporary HOME and XDG directories.
2. Clones `https://github.com/donbeave/essential-mise` into `/tmp/essential-mise-isolated-validation/repository` and checks out `REVIEWED_COMMIT`.
3. Copies `bootstrap.sh` to `/tmp/essential-mise-isolated-validation/bootstrap-local.sh` and changes only its `REPO` assignment to that local repository path. This substitutes local-path adoption without changing adoption, foundation, or verification logic.
4. Uses temporary HOME/XDG locations for every invocation. After local adoption, it sets the checkout's origin metadata to `https://github.com/donbeave/essential-mise.git` to model the expected public checkout for rerun origin checks. It does not fetch from a personal account or alter a real global checkout.
5. For the wrong-origin test, temporarily changes that origin to `https://example.invalid/essential-mise.git`, then restores the expected URL.
6. Records each command's output and exact exit code in `/tmp/essential-mise-evidence/isolated-fixtures`.

## Observed results

Verified with reviewed commit `25854d1dd4157b690c1af6473942472c601f1e81`:

| Invocation | Exit | Captured result |
|---|---:|---|
| `first-run` | 0 | Local adoption, foundation activation, fresh-shell doctor: `activated: yes`, `shims_on_path: yes`, `No problems found` |
| `set-expected-origin` | 0 | Public origin metadata restored for rerun checks |
| `optional-selection-rerun` | 0 | `Existing configuration detected; skipping adoption`, `Foundation only (zsh activation)`, no optional apply, `No problems found` |
| `set-wrong-origin` | 0 | Synthetic wrong origin installed for the safety test |
| `wrong-origin` | 1 | `Existing config has an unexpected or missing origin (https://example.invalid/essential-mise.git); refusing to overwrite or pull.` |
| `restore-origin` | 0 | Expected origin restored |
| `xdg-first-run` | 0 | Adoption into `/tmp/essential-mise-isolated-validation/xdg-custom/mise`, with successful activation |
| `xdg-set-expected-origin` | 0 | Expected public origin set |
| `xdg-fetch` | 0 | Fetch completed |
| `xdg-diff-stat` | 0 | No differences at the reviewed commit |
| `xdg-diff` | 0 | No differences at the reviewed commit |
| `xdg-pull` | 0 | `Already up to date.` |
| `xdg-preview` | 0 | Persistent `foundation,terminal` preview completed |
| `xdg-unapply-preview` | 0 | After selecting only `foundation`, dry-run unapply reported `mise nothing to remove for terminal`; no optional apply was run |

The first replay attempt also proved that an isolated HOME must exist before stage 0 writes `.zprofile`. The fixture now creates both temporary homes explicitly; the successful replay above includes that correction.

Host-collected evidence is under `/tmp/essential-mise-evidence-review/isolated-fixtures`; guest-origin logs and `report.txt` are under `/tmp/essential-mise-evidence/isolated-fixtures`.
