#!/bin/bash
source "$(dirname "$0")/common.sh"

destination="${1:-/tmp/essential-mise-evidence}"
vm_require_name
vm_require_running
vm_scan_host_key
vm_ssh 'mkdir -p /tmp/essential-mise-evidence'
mkdir -p "$destination"
vm_scp -r "$VM_SSH_USER@$VM_IP:/tmp/essential-mise-evidence/." "$destination/"
printf 'Collected guest evidence in %s\n' "$destination"
