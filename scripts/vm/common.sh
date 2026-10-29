#!/bin/bash
set -euo pipefail

VM_NAME="${VM_NAME:-essential-mise-working}"
VM_SSH_USER="${VM_SSH_USER:-admin}"
VM_CONFIG_DIR="${VM_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/essential-mise/vm}"
VM_SSH_KEY="${VM_SSH_KEY:-$VM_CONFIG_DIR/id_ed25519}"
VM_KNOWN_HOSTS="${VM_KNOWN_HOSTS:-$VM_CONFIG_DIR/known_hosts}"
VM_IP=''

fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

vm_require_name() {
  case "$VM_NAME" in
    essential-mise-working|essential-mise-fresh-baseline|essential-mise-automation-baseline) ;;
    essential-mise-test-*)
      [[ "$VM_NAME" =~ ^essential-mise-test-[a-zA-Z0-9_-]+$ ]] || fail "Invalid disposable VM name: $VM_NAME"
      [[ -f "$VM_CONFIG_DIR/disposable-vms/$VM_NAME" && ! -L "$VM_CONFIG_DIR/disposable-vms/$VM_NAME" ]] || \
        fail "No ownership record for disposable VM: $VM_NAME"
      ;;
    *) fail "Refusing unrelated VM name: $VM_NAME" ;;
  esac
}

vm_is_running() {
  tart get "$VM_NAME" --format json 2>/dev/null | /usr/bin/python3 -c 'import json, sys; config = json.load(sys.stdin); raise SystemExit(config.get("Running") is not True)'
}

vm_require_running() {
  vm_is_running || fail "VM is not running: $VM_NAME"
}

vm_wait_ip() {
  local attempt ip
  for attempt in {1..60}; do
    ip="$(tart ip "$VM_NAME" --wait 2 2>/dev/null || true)"
    if [[ -n "$ip" ]]; then
      VM_IP="$ip"
      return 0
    fi
    sleep 2
  done
  fail "Timed out waiting for VM IP: $VM_NAME"
}

vm_require_ip() {
  if [[ -z "$VM_IP" ]]; then
    vm_scan_host_key
  fi
}

vm_scan_host_key() {
  vm_wait_ip
  [[ -r "$VM_SSH_KEY" ]] || fail "Guest-specific SSH key not found: $VM_SSH_KEY"
  mkdir -p "$(dirname "$VM_KNOWN_HOSTS")"
  touch "$VM_KNOWN_HOSTS"
  chmod 600 "$VM_KNOWN_HOSTS"
  if ! /usr/bin/ssh-keygen -F "$VM_IP" -f "$VM_KNOWN_HOSTS" >/dev/null; then
    /usr/bin/ssh-keyscan -T 10 -t ed25519 "$VM_IP" >> "$VM_KNOWN_HOSTS"
  fi
}

vm_ssh() {
  vm_require_ip
  /usr/bin/ssh -T -i "$VM_SSH_KEY" \
    -o BatchMode=yes \
    -o IdentitiesOnly=yes \
    -o ConnectTimeout=10 \
    -o StrictHostKeyChecking=yes \
    -o UserKnownHostsFile="$VM_KNOWN_HOSTS" \
    -o LogLevel=ERROR \
    "$VM_SSH_USER@$VM_IP" "$@"
}

vm_scp() {
  vm_require_ip
  /usr/bin/scp -i "$VM_SSH_KEY" \
    -o BatchMode=yes \
    -o IdentitiesOnly=yes \
    -o ConnectTimeout=10 \
    -o StrictHostKeyChecking=yes \
    -o UserKnownHostsFile="$VM_KNOWN_HOSTS" \
    -o LogLevel=ERROR \
    "$@"
}
