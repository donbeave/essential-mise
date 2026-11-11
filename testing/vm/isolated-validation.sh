#!/bin/bash
set -euo pipefail

fixture_root=/private/tmp/essential-mise-isolated-validation
evidence_dir=/tmp/essential-mise-evidence/isolated-fixtures
source_repo="$fixture_root/repository"
source_script="$fixture_root/bootstrap-local.sh"
config_arg="${ESSENTIAL_MISE_CONFIG:-}"
config_host_dir="${ESSENTIAL_MISE_CONFIG_DIR:-}"
bootstrap_path="$(cd "$(dirname "$0")/../.." && pwd)/bootstrap.sh"
scenario_file=''
usage='Usage: REVIEWED_COMMIT=<config sha> isolated-validation.sh --config <OWNER/REPO|URL> [--config-dir <dir>] [--bootstrap <path>] [--scenario <file>]'
while [[ $# -gt 0 ]]; do
  case "$1" in
    --config) config_arg="$2"; shift 2 ;;
    --config-dir) config_host_dir="$2"; shift 2 ;;
    --bootstrap) bootstrap_path="$2"; shift 2 ;;
    --scenario) scenario_file="$2"; shift 2 ;;
    *) printf '%s\n' "$usage" >&2; exit 1 ;;
  esac
done
[[ -n "$config_arg" ]] || { printf '%s\n' "$usage" >&2; exit 1; }
[[ -f "$bootstrap_path" ]] || { printf 'Bootstrap file not found: %s\n' "$bootstrap_path" >&2; exit 1; }
if [[ -z "$scenario_file" ]]; then
  :
elif [[ ! -f "$scenario_file" && -n "$config_host_dir" && -f "$config_host_dir/tests/vm/scenarios/$scenario_file" ]]; then
  scenario_file="$config_host_dir/tests/vm/scenarios/$scenario_file"
fi
[[ -z "$scenario_file" || -f "$scenario_file" ]] || { printf 'Scenario file not found: %s\n' "$scenario_file" >&2; exit 1; }
case "$config_arg" in
  */*) [[ "$config_arg" == *://* || "$config_arg" == /* ]] && config_url="$config_arg" || config_url="https://github.com/$config_arg.git" ;;
  *) printf 'Invalid --config: %s\n' "$config_arg" >&2; exit 1 ;;
esac
expected_origin="$config_url"
# shellcheck disable=SC1090
[[ -z "$scenario_file" ]] || source "$scenario_file"
update_remote="$fixture_root/update-remote.git"
update_branch=reviewed-update
update_ref="refs/heads/$update_branch"

set_report_status() {
  local status="$1" report_tmp="$evidence_dir/report.txt.tmp"
  [[ -f "$evidence_dir/report.txt" ]] || return 0
  {
    printf '%s\n' "$status"
    /usr/bin/tail -n +2 "$evidence_dir/report.txt"
  } > "$report_tmp"
  /bin/mv "$report_tmp" "$evidence_dir/report.txt"
}
record_failure() {
  local result=$?
  trap - EXIT
  if [[ "$result" -ne 0 ]]; then
    set +e
    set_report_status "FAIL exit=$result"
  fi
}
trap record_failure EXIT

rm -rf "$fixture_root" "$evidence_dir"
mkdir -p "$fixture_root" "$evidence_dir" "$fixture_root/first-home" "$fixture_root/xdg-home"
printf 'IN_PROGRESS\nfixture_root=%s\nsource_repo=%s\nreviewed_commit=%s\n' \
  "$fixture_root" "$source_repo" "${REVIEWED_COMMIT:-}" > "$evidence_dir/report.txt"
[[ "${REVIEWED_COMMIT:-}" =~ ^[0-9a-f]{40}$ ]] || {
  printf 'REVIEWED_COMMIT must be a full reviewed commit SHA\n' >&2
  exit 1
}

git clone "$config_url" "$source_repo" > "$evidence_dir/clone.log" 2>&1
git -C "$source_repo" checkout "$REVIEWED_COMMIT" >> "$evidence_dir/clone.log" 2>&1
/bin/cp "$bootstrap_path" "$source_script"

assert_reviewed_revision() {
  local checkout="$1" actual
  actual="$(/usr/bin/git -C "$checkout" rev-parse HEAD)" || return 1
  printf 'HEAD=%s\n' "$actual"
  [[ "$actual" == "$REVIEWED_COMMIT" ]] || {
    printf 'Expected reviewed HEAD %s\n' "$REVIEWED_COMMIT" >&2
    return 1
  }
}

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
xdg_home="$fixture_root/xdg-home"
first_path="$first_home/.local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"
first_config="$first_home/.config/mise"
run_and_record make-non-git-config 0 /bin/mkdir -p "$first_config"
run_and_record seed-non-git-config 0 /bin/bash -c "printf 'existing-user-setting = true\n' > '$first_config/config.toml'"
run_and_record non-git-refused 1 env HOME="$first_home" PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record non-git-config-preserved 0 /usr/bin/grep -F 'existing-user-setting = true' "$first_config/config.toml"
run_and_record move-non-git-config 0 /bin/mv "$first_config" "$first_home/.config/mise.user-backup"
run_and_record first-run 0 env HOME="$first_home" PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record first-adopted-revision 0 assert_reviewed_revision "$first_config"
run_and_record noninteractive-category-skip 0 /usr/bin/grep -F 'No optional categories selected.' "$evidence_dir/first-run.log"
run_and_record set-expected-origin 0 git -C "$first_config" remote set-url origin "$expected_origin"
initialized_path="/opt/homebrew/bin:/opt/homebrew/sbin:/usr/bin:/bin:/usr/sbin:/sbin"
run_and_record existing-homebrew-empty-shellenv 0 env HOME="$first_home" PATH="$initialized_path" /bin/bash -c 'test -z "$(/opt/homebrew/bin/brew shellenv)"'
run_and_record existing-homebrew-rerun 0 env HOME="$first_home" PATH="$initialized_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record wrap-newer-mise-binary 0 /bin/mv "$first_home/.local/bin/mise" "$first_home/.local/bin/mise-real"
run_and_record write-newer-mise-wrapper 0 /bin/bash -c "printf '#!/bin/bash\nif [[ \"\$1\" == --version || \"\$1\" == version ]]; then\n  printf \"2999.1.1 test\\n\"\nelse\n  exec \"\$HOME/.local/bin/mise-real\" \"\$@\"\nfi\n' > '$first_home/.local/bin/mise'"
run_and_record executable-newer-mise-wrapper 0 /bin/chmod +x "$first_home/.local/bin/mise"
newer_mise_checksum="$(/usr/bin/shasum -a 256 "$first_home/.local/bin/mise")"
run_and_record newer-mise-rerun 0 env HOME="$first_home" PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record newer-mise-preserved 0 /bin/bash -c '[[ "$(/usr/bin/shasum -a 256 "$1")" == "$2" ]]' _ "$first_home/.local/bin/mise" "$newer_mise_checksum"
run_and_record remove-newer-mise-wrapper 0 /bin/rm "$first_home/.local/bin/mise"
run_and_record restore-mise-binary 0 /bin/mv "$first_home/.local/bin/mise-real" "$first_home/.local/bin/mise"
run_and_record optional-selection-rerun 0 env HOME="$first_home" PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record set-wrong-origin 0 git -C "$first_config" remote set-url origin https://example.invalid/essential-mise-config.git
run_and_record wrong-origin 1 env HOME="$first_home" PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
/usr/bin/python3 - "$evidence_dir/wrong-origin.log" <<'PY'
import pathlib
import sys
text = pathlib.Path(sys.argv[1]).read_text()
if 'unexpected or missing origin' not in text or 'refusing to overwrite or pull' not in text:
    raise SystemExit('Wrong-origin refusal text was not found')
PY
run_and_record restore-origin 0 git -C "$first_config" remote set-url origin "$expected_origin"
run_and_record home-alias 1 env HOME="$first_home" MISE_CONFIG_DIR="$first_home/." PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record home-alias-reason 0 /usr/bin/grep -F 'empty or relative path component' "$evidence_dir/home-alias.log"
run_and_record root-alias 1 env HOME="$first_home" MISE_CONFIG_DIR='//' PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record make-symlink-parent 0 /bin/mkdir -p "$fixture_root/actual-config-parent"
run_and_record set-symlink-parent 0 /bin/ln -s "$fixture_root/actual-config-parent" "$fixture_root/config-link"
run_and_record symlink-parent 1 env HOME="$first_home" MISE_CONFIG_DIR="$fixture_root/config-link/mise" PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record symlink-parent-reason 0 /usr/bin/grep -F 'cannot traverse a symlink' "$evidence_dir/symlink-parent.log"
run_and_record symlink-parent-no-adoption 0 /bin/test ! -e "$fixture_root/actual-config-parent/mise"
run_and_record make-bare-config 0 /usr/bin/git init --bare "$fixture_root/bare-config"
run_and_record bare-origin 0 /usr/bin/git -C "$fixture_root/bare-config" remote add origin "$expected_origin"
run_and_record bare-config 1 env HOME="$first_home" MISE_CONFIG_DIR="$fixture_root/bare-config" PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record bare-config-reason 0 /usr/bin/grep -F 'not a Git worktree' "$evidence_dir/bare-config.log"
/bin/mkdir -p "$first_config/subdirectory-fixture"
run_and_record repository-subdirectory 1 env HOME="$first_home" MISE_CONFIG_DIR="$first_config/subdirectory-fixture" PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
run_and_record repository-subdirectory-reason 0 /usr/bin/grep -F 'actual worktree root' "$evidence_dir/repository-subdirectory.log"
run_and_record add-linked-worktree 0 /usr/bin/git -C "$first_config" worktree add --detach "$fixture_root/linked-config" "$REVIEWED_COMMIT"
run_and_record linked-worktree 0 env HOME="$first_home" MISE_CONFIG_DIR="$fixture_root/linked-config" PATH="$first_path" /bin/bash "$source_script" bootstrap --config "$source_repo"

xdg_config="$fixture_root/xdg-custom"
xdg_path="$xdg_home/.local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"
run_and_record xdg-first-run 0 env HOME="$xdg_home" XDG_CONFIG_HOME="$xdg_config" PATH="$xdg_path" /bin/bash "$source_script" bootstrap --config "$source_repo"
xdg_checkout="$xdg_config/mise"
run_and_record xdg-adopted-revision 0 assert_reviewed_revision "$xdg_checkout"
run_and_record make-update-remote 0 /usr/bin/git init --bare "$update_remote"
run_and_record pin-update-ref 0 /usr/bin/git -C "$source_repo" push "$update_remote" "$REVIEWED_COMMIT:$update_ref"
run_and_record xdg-set-update-origin 0 git -C "$xdg_checkout" remote set-url origin "$update_remote"
run_and_record xdg-fetch 0 git -C "$xdg_checkout" fetch origin
run_and_record xdg-reset-for-update 0 git -C "$xdg_checkout" reset --hard "$REVIEWED_COMMIT^"
run_and_record xdg-diff-stat 0 git -C "$xdg_checkout" diff --stat "HEAD..origin/$update_branch"
run_and_record xdg-diff 0 git -C "$xdg_checkout" diff "HEAD..origin/$update_branch"
run_and_record xdg-diff-has-changes 1 git -C "$xdg_checkout" diff --quiet "HEAD..origin/$update_branch"
run_and_record xdg-pull 0 git -C "$xdg_checkout" pull --ff-only origin "$update_branch"
run_and_record xdg-updated-revision 0 assert_reviewed_revision "$xdg_checkout"
if declare -F isolated_scenario >/dev/null; then
  isolated_scenario
fi

assertion_count="$(/usr/bin/grep -Ec '^[^ ]+ exit=[0-9]+ expected=[0-9]+ mise=' "$evidence_dir/report.txt")"
printf 'All %s isolated fixture command-result rows passed. Logs and exact exit codes are in %s\n' \
  "$assertion_count" "$evidence_dir" | tee -a "$evidence_dir/report.txt"
set_report_status PASS
/bin/cat "$evidence_dir/report.txt"
