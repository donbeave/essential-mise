#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
source "$script_dir/common.sh"

vm_require_name
if ! vm_is_running; then
  tart_args=(--no-clipboard)
  if [[ "${VM_GRAPHICS:-0}" == '0' ]]; then
    tart_args+=(--no-graphics)
  elif [[ "${VM_GRAPHICS:-0}" != '1' ]]; then
    fail 'VM_GRAPHICS must be 0 or 1'
  fi
  nohup tart run "${tart_args[@]}" "$VM_NAME" > "/tmp/${VM_NAME}.log" 2>&1 < /dev/null &
fi
vm_wait_ip
vm_scan_host_key
if ! vm_ssh 'true' 2>/dev/null; then
  [[ -n "${VM_OWNER_PASSWORD_FILE:-}" ]] || \
    fail 'Guest SSH key needs restoration; provide VM_OWNER_PASSWORD_FILE'
  "$script_dir/ensure-guest-access.sh"
fi
printf 'VM %s is running at %s\n' "$VM_NAME" "$VM_IP"
