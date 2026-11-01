#!/bin/bash
set -euo pipefail

brew=/opt/homebrew/bin/brew
formula=ampcode
evidence=/tmp/essential-mise-evidence/hook-lifecycle
rm -rf "$evidence"
mkdir -p "$evidence"
printf 'IN_PROGRESS\n' > "$evidence/report.txt"
linked="$($brew --prefix "$formula")"
current="$(readlink "$linked")"
current="${current##*/}"
printf 'original_version=%s\n' "$current" > "$evidence/versions.env"
printf 'tested_version=%s\n' "$current" >> "$evidence/versions.env"
record_failure() {
  local result=$?
  if [[ "$result" -ne 0 ]]; then printf 'FAIL exit=%s\n' "$result" > "$evidence/report.txt"; fi
}
trap record_failure EXIT
"$brew" outdated --formula "$formula" --json > "$evidence/outdated.json" || true
"$brew" info --formula "$formula" --json > "$evidence/formula-info.json"
if ! "$brew" outdated --formula "$formula" | /usr/bin/grep -E '^ampcode([[:space:]]|$)' > "$evidence/outdated.txt"; then
  printf 'BLOCKED: Amp CLI is not outdated according to Homebrew\n' > "$evidence/report.txt"
  trap - EXIT
  exit 2
fi
"$brew" list --versions --formula > "$evidence/packages-before.txt"
readlink "$linked" > "$evidence/link-before.txt"
cd "$HOME/.config/mise"
"$HOME/.local/bin/mise" -E ai-cli bootstrap > "$evidence/apply.log" 2>&1
"$brew" list --versions --formula > "$evidence/packages-after.txt"
readlink "$linked" > "$evidence/link-after.txt"
if "$brew" outdated --formula "$formula" | /usr/bin/grep -E '^ampcode([[:space:]]|$)' > "$evidence/outdated-after.txt"; then
  printf 'Amp CLI is still outdated after apply\n' >&2
  exit 1
fi
! cmp -s "$evidence/link-before.txt" "$evidence/link-after.txt"
/opt/homebrew/bin/amp --version > "$evidence/health.txt" 2>&1
printf 'outdated_package_upgraded=PASS\nhealth_check=PASS\n' > "$evidence/report.txt"
trap - EXIT
/bin/cat "$evidence/report.txt"
