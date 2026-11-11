#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
export VM_NAME=essential-mise-automation-baseline
source "$script_dir/common.sh"

vm_require_name
"$script_dir/check-vm-ready.sh"
"$script_dir/stop-test-vm.sh"
printf 'Verified automation baseline %s is stopped and preserved.\n' "$VM_NAME"
