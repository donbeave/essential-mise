#!/bin/bash
set -euo pipefail
[[ $# -eq 1 ]] || { printf 'Usage: %s <essential-mise-test-NAME>\n' "$0" >&2; exit 1; }
export VM_NAME="$1"
source "$(dirname "$0")/common.sh"

vm_require_name
marker="$VM_CONFIG_DIR/disposable-vms/$VM_NAME"
IFS= read -r ownership < "$marker"
case "$ownership" in
  source=essential-mise-automation-baseline|source=essential-mise-fresh-baseline) ;;
  *) fail "Unexpected disposable VM ownership record: $marker" ;;
esac
if vm_is_running; then
  tart stop "$VM_NAME"
fi
tart delete "$VM_NAME"
/bin/rm -f "$marker"
printf 'Deleted workflow-owned disposable VM %s\n' "$VM_NAME"
