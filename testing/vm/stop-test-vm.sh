#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/common.sh"

vm_require_name
vm_require_running
vm_scan_host_key
vm_ssh 'sync'
tart stop "$VM_NAME"
printf 'VM %s stopped after guest filesystem sync.\n' "$VM_NAME"
