#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
export VM_NAME=essential-mise-fresh-baseline
source "$script_dir/common.sh"

[[ -n "${VM_OWNER_PASSWORD_FILE:-}" && -f "$VM_OWNER_PASSWORD_FILE" && ! -L "$VM_OWNER_PASSWORD_FILE" ]] || \
  fail 'VM_OWNER_PASSWORD_FILE must name a regular, non-symlink credential file'
[[ "$(stat -f '%Lp' "$VM_OWNER_PASSWORD_FILE")" == '600' ]] || \
  fail 'Guest credential file must have mode 600'
vm_require_name
if tart get "$VM_NAME" --format json >/dev/null 2>&1; then
  fail "Refusing to overwrite existing VM: $VM_NAME"
fi

tart create --from-ipsw="${VM_IPSW:-latest}" --disk-size 100 "$VM_NAME"
"$script_dir/provision-fresh-baseline.sh"
