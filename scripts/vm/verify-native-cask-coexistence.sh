#!/bin/bash
set -euo pipefail

brew=/opt/homebrew/bin/brew
evidence=/tmp/essential-mise-evidence/native-cask-coexistence
rm -rf "$evidence"
mkdir -p "$evidence"
printf 'IN_PROGRESS\n' > "$evidence/report.txt"
record_failure() {
  local result=$?
  if [[ "$result" -ne 0 ]]; then printf 'FAIL exit=%s\n' "$result" > "$evidence/report.txt"; fi
}
trap record_failure EXIT
! "$brew" list --cask ghostty >/dev/null 2>&1
[[ ! -e /Applications/Ghostty.app ]]
"$brew" install --cask ghostty > "$evidence/homebrew-install.log" 2>&1
"$brew" list --cask --versions ghostty > "$evidence/owner-before.txt"
cd "$HOME/.config/mise"
"$HOME/.local/bin/mise" -E terminal bootstrap > "$evidence/native-apply.log" 2>&1
"$HOME/.local/bin/mise" -E terminal bootstrap > "$evidence/native-second-run.log" 2>&1
"$brew" list --cask --versions ghostty > "$evidence/owner-after.txt"
cmp "$evidence/owner-before.txt" "$evidence/owner-after.txt"
[[ "$(readlink /opt/homebrew/bin/ghostty)" == '/Applications/Ghostty.app/Contents/MacOS/ghostty' ]]
"$HOME/.local/bin/mise" -E terminal bootstrap status --missing > "$evidence/native-status.log" 2>&1
printf 'existing_homebrew_owner_preserved=PASS\nnative_apply_and_convergence=PASS\n' > "$evidence/report.txt"
trap - EXIT
/bin/cat "$evidence/report.txt"
