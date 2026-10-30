#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/common.sh"

destination="${1:-/tmp/essential-mise-evidence}"
marker="$destination/.essential-mise-evidence-owner"
[[ "$destination" == /tmp/essential-mise-* && "$destination" != *'..'* ]] || \
  fail 'Evidence destination must be a dedicated /tmp/essential-mise-* path'
[[ ! -L "$destination" ]] || fail 'Evidence destination cannot be a symlink'
vm_require_name
vm_require_running
vm_scan_host_key
vm_ssh 'mkdir -p /tmp/essential-mise-evidence'
if [[ -d "$destination" && -n "$(/bin/ls -A "$destination")" ]]; then
  [[ -f "$marker" && ! -L "$marker" && "$(/bin/cat "$marker")" == essential-mise-validation-evidence ]] || \
    fail "Refusing to replace an unrelated nonempty evidence directory: $destination"
  rm -rf "$destination"
elif [[ -e "$destination" && ! -d "$destination" ]]; then
  fail "Evidence destination is not a directory: $destination"
fi
mkdir -p "$destination"
printf 'essential-mise-validation-evidence\n' > "$marker"
chmod 600 "$marker"
vm_scp -r "$VM_SSH_USER@$VM_IP:/tmp/essential-mise-evidence/." "$destination/"
printf 'Collected guest evidence in %s\n' "$destination"
