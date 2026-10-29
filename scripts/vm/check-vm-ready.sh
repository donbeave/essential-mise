#!/bin/bash
source "$(dirname "$0")/common.sh"

vm_require_name
vm_require_running
vm_scan_host_key
vm_ssh 'set -e
driver=/opt/homebrew/bin/cua-driver
printf "Guest macOS: %s (%s)\n" "$(sw_vers -productVersion)" "$(sw_vers -buildVersion)"
test "$(sw_vers -productVersion)" = "27.2"
test "$(sw_vers -buildVersion)" = "26B123"
uname -m
"$driver" --version
"$driver" permissions status --json
"$driver" doctor
"$driver" call list_apps "{}" >/dev/null
mkdir -p /tmp/essential-mise-evidence
"$driver" call get_desktop_state "{}" --screenshot-out-file /tmp/essential-mise-evidence/readiness-desktop.png >/dev/null
printf "Guest automation readiness checks passed.\n"'
