#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
export VM_NAME=essential-mise-automation-baseline
source "$script_dir/common.sh"
homebrew_installer_sha256=5f333bbe53bc490e51e7ccb1df8779b3dd6ee73a1a7379efda216edb08ccb148

vm_require_name
export VM_GRAPHICS=1
"$script_dir/start-test-vm.sh"
if ! vm_ssh 'test -x /opt/homebrew/bin/brew && /opt/homebrew/bin/brew --version >/dev/null'; then
  [[ -n "${VM_OWNER_PASSWORD_FILE:-}" ]] || fail 'Homebrew installation requires VM_OWNER_PASSWORD_FILE'
  installer="$(mktemp /tmp/essential-mise-homebrew-install.XXXXXX)"
  trap '/bin/rm -f "$installer"' EXIT
  curl -fsSL 'https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh' -o "$installer"
  printf '%s  %s\n' "$homebrew_installer_sha256" "$installer" | shasum -a 256 -c -
  /bin/bash -n "$installer"
  vm_scp "$installer" "$VM_SSH_USER@$VM_IP:/tmp/essential-mise-homebrew-install.sh"
  "$script_dir/owner-auth-ssh.sh" \
    'HOMEBREW_NO_ANALYTICS=1 /bin/bash /tmp/essential-mise-homebrew-install.sh'
fi
vm_ssh 'set -e
export HOMEBREW_NO_ANALYTICS=1
/opt/homebrew/bin/brew install --cask cuadriver
/opt/homebrew/bin/cua-driver --version'
printf 'Automation VM is ready for guest-only consent. Run grant-cua-permissions.sh, then finalize-automation-baseline.sh.\n'
