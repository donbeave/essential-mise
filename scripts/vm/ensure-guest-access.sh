#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
source "$script_dir/common.sh"

vm_require_name
vm_require_running
vm_scan_host_key
if vm_ssh 'true' 2>/dev/null; then
  printf 'Guest SSH key access verified for %s.\n' "$VM_NAME"
  exit 0
fi
[[ -n "${VM_OWNER_PASSWORD_FILE:-}" ]] || \
  fail 'Dedicated key is not installed in this clone; provide VM_OWNER_PASSWORD_FILE to restore it'
IFS= read -r public_key < "$VM_SSH_KEY.pub"
"$script_dir/owner-auth-ssh.sh" \
  "umask 077; mkdir -p ~/.ssh; printf '%s\n' '$public_key' >> ~/.ssh/authorized_keys"
vm_ssh 'true'
printf 'Guest SSH key access restored and verified for %s.\n' "$VM_NAME"
