#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/common.sh"

usage='Usage: VM_NAME=essential-mise-test-NAME run-test-scenario.sh [--bootstrap <local path>] [--config <OWNER/REPO|file://...>] [--config-dir <host dir>] (--scenario <file> | stage0)'
bootstrap_file=''
config_arg="${ESSENTIAL_MISE_CONFIG:-}"
config_host_dir="${ESSENTIAL_MISE_CONFIG_DIR:-}"
scenario_file=''
scenario=''
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bootstrap) [[ $# -ge 2 ]] || fail "$usage"; bootstrap_file="$2"; shift 2 ;;
    --config) [[ $# -ge 2 ]] || fail "$usage"; config_arg="$2"; shift 2 ;;
    --config-dir) [[ $# -ge 2 ]] || fail "$usage"; config_host_dir="$2"; shift 2 ;;
    --scenario) [[ $# -ge 2 ]] || fail "$usage"; scenario_file="$2"; shift 2 ;;
    stage0) scenario=stage0; shift ;;
    *) fail "$usage" ;;
  esac
done
if [[ -n "$scenario_file" && "$scenario" == stage0 ]]; then fail "$usage"; fi
if [[ -n "$scenario_file" ]]; then
  if [[ ! -f "$scenario_file" && -n "$config_host_dir" && -f "$config_host_dir/tests/vm/scenarios/$scenario_file" ]]; then
    scenario_file="$config_host_dir/tests/vm/scenarios/$scenario_file"
  fi
  [[ -f "$scenario_file" ]] || fail "Scenario file not found: $scenario_file"
  scenario="$(basename "$scenario_file" .sh)"
elif [[ "$scenario" != stage0 ]]; then
  fail "$usage"
fi
[[ -n "$config_arg" ]] || fail 'A config source is required: --config OWNER/REPO or ESSENTIAL_MISE_CONFIG'
if [[ -n "$bootstrap_file" ]]; then
  [[ -f "$bootstrap_file" ]] || fail "Bootstrap file not found: $bootstrap_file"
fi
case "$VM_NAME" in
  essential-mise-test-*) ;;
  *) fail 'Application scenarios require a registered disposable VM' ;;
esac
vm_require_name
vm_require_running
vm_scan_host_key
evidence_dir="/tmp/essential-mise-evidence/$scenario"
vm_ssh "rm -rf '$evidence_dir' && mkdir -p '$evidence_dir'"
engine_repo="${ESSENTIAL_MISE_ENGINE_REPO:-donbeave/essential-mise}"
reviewed_commit="${VM_REVIEWED_COMMIT:-}"
if [[ -z "$bootstrap_file" ]]; then
  reviewed_commit="${reviewed_commit:-$(git rev-parse HEAD)}"
  [[ "$reviewed_commit" =~ ^[0-9a-f]{40}$ ]] || fail 'A full reviewed commit SHA is required'
fi
config_dir="/Users/$VM_SSH_USER/.config/mise"
fixture_sha256="$(shasum -a 256 "$0" | cut -d' ' -f1)"
scenario_sha256=none
[[ -z "$scenario_file" ]] || scenario_sha256="$(shasum -a 256 "$scenario_file" | cut -d' ' -f1)"
bootstrap_sha256=none
[[ -z "$bootstrap_file" ]] || bootstrap_sha256="$(shasum -a 256 "$bootstrap_file" | cut -d' ' -f1)"
vm_ssh "printf 'scenario=%s\nreviewed_commit=%s\nvm_name=%s\nfixture_sha256=%s\nscenario_sha256=%s\nbootstrap_sha256=%s\ntart_version=%s\n' '$scenario' '$reviewed_commit' '$VM_NAME' '$fixture_sha256' '$scenario_sha256' '$bootstrap_sha256' '$(tart --version)' > '$evidence_dir/scenario.env'"
vm_ssh "printf 'product_version=%s\nbuild_version=%s\n' \"\$(sw_vers -productVersion)\" \"\$(sw_vers -buildVersion)\" > '$evidence_dir/guest-os.txt'"
vm_scp "$0" "$VM_SSH_USER@$VM_IP:$evidence_dir/run-test-scenario.sh"
[[ -z "$scenario_file" ]] || vm_scp "$scenario_file" "$VM_SSH_USER@$VM_IP:$evidence_dir/scenario.sh"
if vm_ssh 'test -x "$HOME/.local/bin/mise"'; then
  vm_ssh "\$HOME/.local/bin/mise --version > '$evidence_dir/mise-version.txt'"
fi

record_config_state() {
  local prefix="$1" actual_commit actual_top inside_worktree is_bare
  local actual_commit_q actual_top_q config_q evidence_q physical_config physical_top
  printf -v config_q '%q' "$config_dir"
  printf -v evidence_q '%q' "$evidence_dir"
  vm_ssh "rm -f $evidence_q/$prefix-revision.txt $evidence_q/$prefix-status.txt"
  vm_ssh "test -d $config_q" || fail "Guest config is absent: $config_dir"
  is_bare="$(vm_ssh "git -C $config_q rev-parse --is-bare-repository" 2>/dev/null)" || \
    fail "Guest config is not a Git repository: $config_dir"
  [[ "$is_bare" == false ]] || fail "Guest config must be a nonbare Git worktree: $config_dir"
  inside_worktree="$(vm_ssh "git -C $config_q rev-parse --is-inside-work-tree" 2>/dev/null)" || \
    fail "Guest config is not a Git worktree: $config_dir"
  [[ "$inside_worktree" == true ]] || fail "Guest config is not a Git worktree: $config_dir"
  actual_top="$(vm_ssh "git -C $config_q rev-parse --show-toplevel" 2>/dev/null)" || \
    fail "Cannot determine guest config worktree root: $config_dir"
  printf -v actual_top_q '%q' "$actual_top"
  physical_config="$(vm_ssh "cd -P $config_q && pwd -P")" || \
    fail "Cannot resolve guest config directory: $config_dir"
  physical_top="$(vm_ssh "cd -P $actual_top_q && pwd -P")" || \
    fail "Cannot resolve guest config worktree root: $actual_top"
  [[ "$physical_top" == "$physical_config" ]] || \
    fail "Guest config worktree root $actual_top does not match $config_dir"
  actual_commit="$(vm_ssh "git -C $config_q rev-parse HEAD" 2>/dev/null)" || \
    fail "Cannot determine guest config HEAD: $config_dir"
  [[ "$actual_commit" =~ ^[0-9a-f]{40}$ ]] || fail "Guest config returned an invalid HEAD: $actual_commit"
  printf -v actual_commit_q '%q' "$actual_commit"
  vm_ssh "printf '%s\n' $actual_commit_q > $evidence_q/$prefix-revision.txt"
  vm_ssh "git -C $config_q status --porcelain > $evidence_q/$prefix-status.txt" || \
    fail "Cannot determine guest config cleanliness: $config_dir"
  if [[ -n "${VM_CONFIG_COMMIT:-}" ]]; then
    [[ "$actual_commit" == "$VM_CONFIG_COMMIT" ]] || \
      fail "Guest config $actual_commit does not match expected config commit $VM_CONFIG_COMMIT"
  fi
  vm_ssh "test ! -s $evidence_q/$prefix-status.txt" || fail 'Guest config has uncommitted changes'
}


run_logged() {
  local name="$1" command="$2" result
  set +e
  vm_ssh "set -o pipefail; { $command; } 2>&1 | tee '$evidence_dir/$name.log'"
  result=$?
  set -e
  vm_ssh "printf '%s\n' '$result' > '$evidence_dir/$name.exit'"
  return "$result"
}

run_logged_interactive() {
  local name="$1" command="$2" result
  local host_evidence="/tmp/essential-mise-host-evidence/$scenario"
  mkdir -p "$host_evidence"
  set +e
  VM_OWNER_PASSWORD_FILE="${VM_OWNER_PASSWORD_FILE:-/tmp/essential-mise-vm-owner-password}" \
  VM_AUTO_CONFIRM="${VM_AUTO_CONFIRM:-0}" \
    "$(dirname "$0")/owner-auth-ssh.sh" "$command" 2>&1 | tee "$host_evidence/$name.log"
  result=$?
  set -e
  vm_scp "$host_evidence/$name.log" "$VM_SSH_USER@$VM_IP:$evidence_dir/$name.log"
  vm_ssh "printf '%s\n' '$result' > '$evidence_dir/$name.exit'"
  return "$result"
}

record_adopted_state() {
  record_config_state adopted-config
  vm_ssh "test -x \"\$HOME/.local/bin/mise\" && \$HOME/.local/bin/mise --version > '$evidence_dir/adopted-mise-version.txt'"
}


if [[ -n "$bootstrap_file" ]]; then
  vm_scp "$bootstrap_file" "$VM_SSH_USER@$VM_IP:$evidence_dir/bootstrap.sh"
  stage0_command="set -e; /bin/bash '$evidence_dir/bootstrap.sh' bootstrap --config '$config_arg'"
else
  stage0_command="set -e
reviewed_commit='$reviewed_commit'
/bin/bash -c \"\$(/usr/bin/curl -fsSL \"https://raw.githubusercontent.com/$engine_repo/\$reviewed_commit/bootstrap.sh\")\" bootstrap --config '$config_arg'"
fi

if [[ "$scenario" == stage0 ]]; then
  run_logged cold-preconditions "test ! -e '$config_dir' && test ! -x /opt/homebrew/bin/brew && { /usr/bin/xcode-select -p >/dev/null 2>&1 && printf 'command_line_tools=present\n' || printf 'command_line_tools=absent\n'; } && printf 'homebrew=absent\nconfig=absent\n'"
  VM_AUTO_CONFIRM=1 run_logged_interactive cold-first-run "$stage0_command"
  record_adopted_state
  run_logged first-run-stage "/usr/bin/grep -E 'stage 6/6' '$evidence_dir/cold-first-run.log'"
  run_logged rerun "$stage0_command"
  original_origin="$(vm_ssh "git -C '$config_dir' remote get-url origin")"
  vm_ssh "git -C '$config_dir' remote set-url origin 'https://example.invalid/essential-mise-config.git'"
  if run_logged wrong-origin "$stage0_command"; then
    outcome=unexpected-success
  else
    outcome=expected-refusal
  fi
  vm_ssh "git -C '$config_dir' remote set-url origin '$original_origin'"
  [[ "$outcome" == expected-refusal ]] || fail 'Stage 0 accepted an unexpected origin'
else
  record_config_state config
  # shellcheck disable=SC1090
  source "$scenario_file"
fi
printf 'Guest scenario %s completed on %s. Review logs before assigning release status.\n' "$scenario" "$VM_NAME"
