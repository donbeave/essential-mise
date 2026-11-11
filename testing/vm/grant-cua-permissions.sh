#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
source "$script_dir/common.sh"

vm_require_name
if ! vm_is_running; then
  export VM_GRAPHICS=1
  "$script_dir/start-test-vm.sh"
else
  vm_scan_host_key
fi
vm_ssh 'if [ -d /Applications/CuaDriver.app ]; then
  pkill -x cua-driver || true
  sleep 2
  /usr/bin/open -n -g -a CuaDriver --args serve
  sleep 5
fi
"$ESSENTIAL_MISE_CUA_DRIVER" permissions grant
"$ESSENTIAL_MISE_CUA_DRIVER" permissions status --json
"$ESSENTIAL_MISE_CUA_DRIVER" call list_apps "{}" >/dev/null'
