#!/bin/bash
source "$(dirname "$0")/common.sh"

vm_require_name
vm_require_running
vm_scan_host_key
vm_ssh '/opt/homebrew/bin/cua-driver mcp'
