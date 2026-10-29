#!/bin/bash
source "$(dirname "$0")/common.sh"

vm_require_name
vm_require_running
vm_scan_host_key
vm_ssh '"$ESSENTIAL_MISE_CUA_DRIVER" mcp'
