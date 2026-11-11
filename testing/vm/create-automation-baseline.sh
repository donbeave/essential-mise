#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
export VM_NAME=essential-mise-automation-baseline
source "$script_dir/common.sh"
source_vm=essential-mise-fresh-baseline

vm_require_name
if tart get "$VM_NAME" --format json >/dev/null 2>&1; then
  fail "Refusing to overwrite existing VM: $VM_NAME"
fi
if VM_NAME="$source_vm" vm_is_running; then
  fail "Source baseline must be stopped: $source_vm"
fi
tart get "$source_vm" --format json >/dev/null
tart clone "$source_vm" "$VM_NAME"
tart set "$VM_NAME" --cpu 4 --memory 8192 --display 1512x982
"$script_dir/prepare-automation-baseline.sh"
