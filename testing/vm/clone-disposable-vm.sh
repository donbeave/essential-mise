#!/bin/bash
set -euo pipefail
script_dir="$(dirname "$0")"
source "$script_dir/common.sh"

[[ $# -eq 1 ]] || fail "Usage: $0 <essential-mise-test-NAME>"
name="$1"
[[ "$name" =~ ^essential-mise-test-[a-zA-Z0-9_-]+$ ]] || fail 'Invalid disposable VM name'
case "$name" in
  *baseline*|*working*) fail 'Refusing baseline or working name for a disposable VM' ;;
esac
if tart get "$name" --format json >/dev/null 2>&1; then
  fail "Refusing to overwrite existing VM: $name"
fi
source_vm="${VM_SOURCE_BASELINE:-essential-mise-automation-baseline}"
case "$source_vm" in
  essential-mise-automation-baseline|essential-mise-fresh-baseline) ;;
  *) fail "Refusing unexpected source baseline: $source_vm" ;;
esac
if VM_NAME="$source_vm" vm_is_running; then
  fail "Source baseline must be stopped: $source_vm"
fi
tart get "$source_vm" --format json >/dev/null
marker_dir="$VM_CONFIG_DIR/disposable-vms"
marker="$marker_dir/$name"
[[ ! -e "$marker" && ! -L "$marker" ]] || fail "Disposable VM ownership record already exists: $marker"
mkdir -p "$marker_dir"
chmod 700 "$marker_dir"

tart clone "$source_vm" "$name"
tart set "$name" --cpu 4 --memory 8192
printf 'source=%s\n' "$source_vm" > "$marker"
chmod 600 "$marker"
printf 'Created disposable VM %s from %s.\n' "$name" "$source_vm"
