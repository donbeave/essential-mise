#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
export VM_NAME=essential-mise-fresh-baseline
source "$script_dir/common.sh"

[[ -n "${VM_OWNER_PASSWORD_FILE:-}" && -f "$VM_OWNER_PASSWORD_FILE" && ! -L "$VM_OWNER_PASSWORD_FILE" ]] || \
  fail 'VM_OWNER_PASSWORD_FILE must name a regular, non-symlink credential file'
[[ "$(stat -f '%Lp' "$VM_OWNER_PASSWORD_FILE")" == '600' ]] || \
  fail 'Guest credential file must have mode 600'
[[ -r "$VM_SSH_KEY" && -r "$VM_SSH_KEY.pub" ]] || \
  fail 'Generate the dedicated guest SSH identity before provisioning'
vm_require_name
tart get "$VM_NAME" --format json >/dev/null

IFS= read -r owner_password < "$VM_OWNER_PASSWORD_FILE"
[[ -n "$owner_password" ]] || fail 'Guest credential file is empty'
if ! vm_is_running; then
  provisioning_options="fullName=Essential Mise Test,username=$VM_SSH_USER,password=$owner_password,logsInAutomatically=true,enablesRemoteLogin=true"
  tart set "$VM_NAME" --cpu 4 --memory 8192 --display 1512x982
  nohup tart run --no-graphics --no-clipboard --provisioning-opts "$provisioning_options" "$VM_NAME" \
    > "/tmp/$VM_NAME.log" 2>&1 < /dev/null &
  unset provisioning_options
fi
unset owner_password

vm_scan_host_key
"$script_dir/ensure-guest-access.sh"
vm_ssh 'set -e
printf "Guest macOS: %s (%s)\n" "$(sw_vers -productVersion)" "$(sw_vers -buildVersion)"
test "$(sw_vers -productVersion)" = "27.2"
test "$(sw_vers -buildVersion)" = "26B123"
test "$(uname -m)" = "arm64"
csrutil status
spctl --status'
"$script_dir/stop-test-vm.sh"
printf 'Fresh stock-security baseline %s is stopped and preserved.\n' "$VM_NAME"
