#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/common.sh"

[[ $# -eq 2 ]] || fail 'Usage: VM_NAME=<name> cua-call.sh <tool-name> <json-arguments>'
[[ "$1" =~ ^[a-z_]+$ ]] || fail "Invalid Cua tool name: $1"
vm_require_name
vm_require_running
vm_scan_host_key
vm_ssh '/usr/bin/caffeinate -u -t 1; mkdir -p /tmp/essential-mise-evidence/ui-smoke'
printf '%s\n' "$2" | vm_ssh "\"$VM_CUA_DRIVER\" call '$1'"
