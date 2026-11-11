#!/bin/bash
source "$(dirname "$0")/common.sh"

[[ $# -eq 1 ]] || fail "Usage: $0 <artifact-file-or-directory>"
artifact="$1"
[[ -e "$artifact" ]] || fail "Artifact does not exist: $artifact"

vm_require_name
vm_require_running
vm_scan_host_key
vm_ssh 'rm -rf /tmp/essential-mise-artifact && mkdir -p /tmp/essential-mise-artifact'
vm_scp -r "$artifact" "$VM_SSH_USER@$VM_IP:/tmp/essential-mise-artifact/"
printf 'Transferred %s to guest /tmp/essential-mise-artifact/\n' "$artifact"
