#!/bin/bash
set -euo pipefail

[[ "${REVIEWED_COMMIT:-}" =~ ^[0-9a-f]{40}$ ]] || {
  printf 'REVIEWED_COMMIT must be a full reviewed commit SHA\n' >&2
  exit 1
}
fixture_root=/private/tmp/essential-mise-isolated-validation
evidence_dir=/tmp/essential-mise-evidence/isolated-fixtures
source_repo="$fixture_root/repository"
source_script="$fixture_root/bootstrap-local.sh"
expected_origin=https://github.com/donbeave/essential-mise.git

rm -rf "$fixture_root" "$evidence_dir"
mkdir -p "$fixture_root" "$evidence_dir" "$fixture_root/first-home" "$fixture_root/xdg-home"
printf 'IN_PROGRESS\nfixture_root=%s\nsource_repo=%s\nreviewed_commit=%s\n' \
  "$fixture_root" "$source_repo" "$REVIEWED_COMMIT" > "$evidence_dir/report.txt"
record_failure() {
  local result=$?
  if [[ "$result" -ne 0 ]]; then
    printf 'FAIL exit=%s\n' "$result" > "$evidence_dir/report.txt"
  fi
}
trap record_failure EXIT

git clone https://github.com/donbeave/essential-mise "$source_repo" > "$evidence_dir/clone.log" 2>&1
git -C "$source_repo" checkout "$REVIEWED_COMMIT" >> "$evidence_dir/clone.log" 2>&1
/usr/bin/python3 - "$source_repo/bootstrap.sh" "$source_script" "$source_repo" <<'PY'
import pathlib
import sys
source, destination, repository = sys.argv[1:]
text = pathlib.Path(source).read_text()
original = "REPO='donbeave/essential-mise'"
replacement = f"REPO='{repository}'"
if text.count(original) != 1:
    raise SystemExit('Unexpected REPO declaration in bootstrap.sh')
pathlib.Path(destination).write_text(text.replace(original, replacement))
PY

run_and_record() {
  local name="$1" expected="$2"
  shift 2
  set +e
  "$@" > "$evidence_dir/$name.log" 2>&1
  local result=$?
  set -e
  local mise_version=''
  if [[ -x "$first_home/.local/bin/mise" ]]; then
    mise_version="$("$first_home/.local/bin/mise" --version)"
  elif [[ -x "$xdg_home/.local/bin/mise" ]]; then
    mise_version="$("$xdg_home/.local/bin/mise" --version)"
  fi
  printf '%s exit=%s expected=%s mise=%s\n' "$name" "$result" "$expected" "$mise_version" | tee -a "$evidence_dir/report.txt"
  if [[ "$result" -ne "$expected" ]]; then
    /bin/cat "$evidence_dir/$name.log"
    exit 1
  fi
}

first_home="$fixture_root/first-home"
first_path="$first_home/.local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"
run_and_record first-run 0 env HOME="$first_home" PATH="$first_path" /bin/bash "$source_script"
first_config="$first_home/.config/mise"
run_and_record first-adopted-revision 0 git -C "$first_config" diff --quiet "$REVIEWED_COMMIT"
run_and_record set-expected-origin 0 git -C "$first_config" remote set-url origin "$expected_origin"
initialized_path="/opt/homebrew/bin:/opt/homebrew/sbin:/usr/bin:/bin:/usr/sbin:/sbin"
run_and_record existing-homebrew-empty-shellenv 0 env HOME="$first_home" PATH="$initialized_path" /bin/bash -c 'test -z "$(/opt/homebrew/bin/brew shellenv)"'
run_and_record existing-homebrew-rerun 0 env HOME="$first_home" PATH="$initialized_path" /bin/bash "$source_script"
printf 'env = ["ai-cli", "system-preferences"]\n' > "$first_config/miserc.local.toml"
run_and_record optional-selection-rerun 0 env HOME="$first_home" PATH="$first_path" /bin/bash "$source_script"
run_and_record set-wrong-origin 0 git -C "$first_config" remote set-url origin https://example.invalid/essential-mise.git
run_and_record wrong-origin 1 env HOME="$first_home" PATH="$first_path" /bin/bash "$source_script"
/usr/bin/python3 - "$evidence_dir/wrong-origin.log" <<'PY'
import pathlib
import sys
text = pathlib.Path(sys.argv[1]).read_text()
if 'unexpected or missing origin' not in text or 'refusing to overwrite or pull' not in text:
    raise SystemExit('Wrong-origin refusal text was not found')
PY
run_and_record restore-origin 0 git -C "$first_config" remote set-url origin "$expected_origin"
run_and_record home-alias 1 env HOME="$first_home" MISE_CONFIG_DIR="$first_home/." PATH="$first_path" /bin/bash "$source_script"
run_and_record home-alias-reason 0 /usr/bin/grep -F 'empty or relative path component' "$evidence_dir/home-alias.log"
run_and_record root-alias 1 env HOME="$first_home" MISE_CONFIG_DIR='//' PATH="$first_path" /bin/bash "$source_script"
run_and_record make-symlink-parent 0 /bin/mkdir -p "$fixture_root/actual-config-parent"
run_and_record set-symlink-parent 0 /bin/ln -s "$fixture_root/actual-config-parent" "$fixture_root/config-link"
run_and_record symlink-parent 1 env HOME="$first_home" MISE_CONFIG_DIR="$fixture_root/config-link/mise" PATH="$first_path" /bin/bash "$source_script"
run_and_record symlink-parent-reason 0 /usr/bin/grep -F 'cannot traverse a symlink' "$evidence_dir/symlink-parent.log"
run_and_record symlink-parent-no-adoption 0 /usr/bin/test ! -e "$fixture_root/actual-config-parent/mise"
run_and_record make-bare-config 0 /usr/bin/git init --bare "$fixture_root/bare-config"
run_and_record bare-origin 0 /usr/bin/git -C "$fixture_root/bare-config" remote add origin "$expected_origin"
run_and_record bare-config 1 env HOME="$first_home" MISE_CONFIG_DIR="$fixture_root/bare-config" PATH="$first_path" /bin/bash "$source_script"
run_and_record bare-config-reason 0 /usr/bin/grep -F 'not a Git worktree' "$evidence_dir/bare-config.log"
run_and_record repository-subdirectory 1 env HOME="$first_home" MISE_CONFIG_DIR="$first_config/docs" PATH="$first_path" /bin/bash "$source_script"
run_and_record repository-subdirectory-reason 0 /usr/bin/grep -F 'actual worktree root' "$evidence_dir/repository-subdirectory.log"
run_and_record add-linked-worktree 0 /usr/bin/git -C "$first_config" worktree add --detach "$fixture_root/linked-config" "$REVIEWED_COMMIT"
run_and_record linked-worktree 0 env HOME="$first_home" MISE_CONFIG_DIR="$fixture_root/linked-config" PATH="$first_path" /bin/bash "$source_script"

xdg_home="$fixture_root/xdg-home"
xdg_config="$fixture_root/xdg-custom"
xdg_path="$xdg_home/.local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"
run_and_record xdg-first-run 0 env HOME="$xdg_home" XDG_CONFIG_HOME="$xdg_config" PATH="$xdg_path" /bin/bash "$source_script"
xdg_checkout="$xdg_config/mise"
run_and_record xdg-adopted-revision 0 git -C "$xdg_checkout" diff --quiet "$REVIEWED_COMMIT"
run_and_record xdg-set-expected-origin 0 git -C "$xdg_checkout" remote set-url origin "$expected_origin"
run_and_record xdg-fetch 0 git -C "$xdg_checkout" fetch origin
run_and_record xdg-reset-for-update 0 git -C "$xdg_checkout" reset --hard "$REVIEWED_COMMIT^"
run_and_record xdg-diff-stat 0 git -C "$xdg_checkout" diff --stat HEAD..origin/main
run_and_record xdg-diff 0 git -C "$xdg_checkout" diff HEAD..origin/main
run_and_record xdg-diff-has-changes 1 git -C "$xdg_checkout" diff --quiet HEAD..origin/main
run_and_record xdg-pull 0 git -C "$xdg_checkout" pull --ff-only origin main
run_and_record xdg-updated-revision 0 git -C "$xdg_checkout" diff --quiet "$REVIEWED_COMMIT"
printf 'env = ["foundation", "terminal"]\n' > "$xdg_checkout/miserc.local.toml"
run_and_record xdg-preview 0 env HOME="$xdg_home" XDG_CONFIG_HOME="$xdg_config" PATH="$xdg_path" "$xdg_home/.local/bin/mise" -C "$xdg_checkout" bootstrap --dry-run
run_and_record xdg-apply 0 env HOME="$xdg_home" XDG_CONFIG_HOME="$xdg_config" PATH="$xdg_path" "$xdg_home/.local/bin/mise" -C "$xdg_checkout" -E terminal bootstrap --only dotfiles
run_and_record xdg-verify-applied 0 /usr/bin/test -f "$xdg_home/.config/ghostty/config"
run_and_record xdg-shell-persistence 0 env HOME="$xdg_home" XDG_CONFIG_HOME="$xdg_config" PATH="$xdg_path" /bin/zsh -lic 'cd /private/tmp && mise bootstrap --dry-run'
printf 'env = ["foundation"]\n' > "$xdg_checkout/miserc.local.toml"
run_and_record xdg-unapply-preview 0 env HOME="$xdg_home" XDG_CONFIG_HOME="$xdg_config" PATH="$xdg_path" "$xdg_home/.local/bin/mise" -C "$xdg_checkout" bootstrap unapply terminal --dry-run
run_and_record xdg-unapply 0 env HOME="$xdg_home" XDG_CONFIG_HOME="$xdg_config" PATH="$xdg_path" "$xdg_home/.local/bin/mise" -C "$xdg_checkout" bootstrap unapply terminal --yes
run_and_record xdg-verify-removed 0 /usr/bin/test ! -e "$xdg_home/.config/ghostty/config"

printf 'All isolated fixture assertions passed. Logs and exact exit codes are in %s\n' "$evidence_dir" | tee -a "$evidence_dir/report.txt"
/bin/cat "$evidence_dir/report.txt"
